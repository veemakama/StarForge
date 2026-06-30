use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ComplianceSeverity {
    Info,
    Warning,
    Blocking,
}

impl std::fmt::Display for ComplianceSeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ComplianceSeverity::Info => write!(f, "info"),
            ComplianceSeverity::Warning => write!(f, "warning"),
            ComplianceSeverity::Blocking => write!(f, "blocking"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompliancePolicy {
    pub id: String,
    pub name: String,
    pub description: String,
    pub policy_type: PolicyType,
    pub severity: ComplianceSeverity,
    pub enabled: bool,
    pub config: HashMap<String, String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum PolicyType {
    RequiredApprovers,
    DeploymentWindow,
    MaxDeploymentFrequency,
    NetworkRestriction,
    FreezePeriod,
    Custom(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceCheckResult {
    pub policy_id: String,
    pub policy_name: String,
    pub passed: bool,
    pub severity: ComplianceSeverity,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceReport {
    pub request_id: String,
    pub contract_id: String,
    pub network: String,
    pub checks: Vec<ComplianceCheckResult>,
    pub timestamp: String,
    pub all_passed: bool,
    pub blocking_count: usize,
    pub warning_count: usize,
}

fn compliance_dir() -> Result<PathBuf> {
    let dir = crate::utils::config::config_dir().join("compliance");
    if !dir.exists() {
        fs::create_dir_all(&dir)?;
    }
    Ok(dir)
}

fn policies_path() -> Result<PathBuf> {
    Ok(compliance_dir()?.join("policies.json"))
}

fn reports_path() -> Result<PathBuf> {
    Ok(compliance_dir()?.join("reports.json"))
}

fn load_policies_raw() -> Result<Vec<CompliancePolicy>> {
    let path = policies_path()?;
    if !path.exists() {
        return Ok(vec![]);
    }
    let data = fs::read_to_string(&path)?;
    Ok(serde_json::from_str(&data).unwrap_or_default())
}

fn save_policies_raw(policies: &[CompliancePolicy]) -> Result<()> {
    let path = policies_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, serde_json::to_string_pretty(policies)?)?;
    Ok(())
}

fn load_reports_raw() -> Result<Vec<ComplianceReport>> {
    let path = reports_path()?;
    if !path.exists() {
        return Ok(vec![]);
    }
    let data = fs::read_to_string(&path)?;
    Ok(serde_json::from_str(&data).unwrap_or_default())
}

fn save_reports_raw(reports: &[ComplianceReport]) -> Result<()> {
    let path = reports_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, serde_json::to_string_pretty(reports)?)?;
    Ok(())
}

pub fn create_policy(
    name: &str,
    description: &str,
    policy_type: PolicyType,
    severity: ComplianceSeverity,
    config: HashMap<String, String>,
) -> Result<CompliancePolicy> {
    let mut policies = load_policies_raw()?;
    let id = format!(
        "pol-{}",
        uuid::Uuid::new_v4()
            .to_string()
            .split('-')
            .next()
            .unwrap_or("0000")
    );
    let now = Utc::now().to_rfc3339();

    let policy = CompliancePolicy {
        id: id.clone(),
        name: name.to_string(),
        description: description.to_string(),
        policy_type,
        severity,
        enabled: true,
        config,
        created_at: now.clone(),
        updated_at: now,
    };

    policies.push(policy);
    save_policies_raw(&policies)?;
    Ok(policy)
}

pub fn list_policies() -> Result<Vec<CompliancePolicy>> {
    load_policies_raw()
}

pub fn run_compliance_checks(
    request_id: &str,
    contract_id: &str,
    network: &str,
    requested_by: &str,
) -> Result<ComplianceReport> {
    let policies = load_policies_raw();
    let policies = match policies {
        Ok(p) => p.into_iter().filter(|p| p.enabled).collect::<Vec<_>>(),
        Err(_) => vec![],
    };

    let mut checks: Vec<ComplianceCheckResult> = Vec::new();

    for policy in &policies {
        let result = match &policy.policy_type {
            PolicyType::RequiredApprovers => check_required_approvers(policy, network),
            PolicyType::DeploymentWindow => check_deployment_window(policy),
            PolicyType::MaxDeploymentFrequency => {
                check_deployment_frequency(policy, network, requested_by)
            }
            PolicyType::NetworkRestriction => check_network_restriction(policy, network),
            PolicyType::FreezePeriod => check_freeze_period(policy),
            PolicyType::Custom(_) => ComplianceCheckResult {
                policy_id: policy.id.clone(),
                policy_name: policy.name.clone(),
                passed: true,
                severity: ComplianceSeverity::Info,
                message: format!("Custom policy '{}': manual check required", policy.name),
            },
        };
        checks.push(result);
    }

    let blocking_count = checks
        .iter()
        .filter(|c| !c.passed && matches!(c.severity, ComplianceSeverity::Blocking))
        .count();
    let warning_count = checks
        .iter()
        .filter(|c| !c.passed && matches!(c.severity, ComplianceSeverity::Warning))
        .count();
    let all_passed = blocking_count == 0;

    let report = ComplianceReport {
        request_id: request_id.to_string(),
        contract_id: contract_id.to_string(),
        network: network.to_string(),
        checks,
        timestamp: Utc::now().to_rfc3339(),
        all_passed,
        blocking_count,
        warning_count,
    };

    let mut reports = load_reports_raw()?;
    reports.push(report.clone());
    save_reports_raw(&reports)?;

    crate::utils::audit::log_action(
        "compliance_check",
        "system",
        "compliance_report",
        request_id,
        [
            ("contract_id".to_string(), contract_id.to_string()),
            ("network".to_string(), network.to_string()),
            ("all_passed".to_string(), all_passed.to_string()),
            ("blocking".to_string(), blocking_count.to_string()),
        ]
        .into_iter()
        .collect(),
        all_passed,
        if all_passed {
            None
        } else {
            Some("Compliance check failed".to_string())
        },
    )?;

    Ok(report)
}

pub fn get_recent_reports(limit: usize) -> Result<Vec<ComplianceReport>> {
    let mut reports = load_reports_raw()?;
    reports.reverse();
    Ok(reports.into_iter().take(limit).collect())
}

fn check_required_approvers(policy: &CompliancePolicy, network: &str) -> ComplianceCheckResult {
    let min_approvers = policy
        .config
        .get("min_approvers")
        .map(|v| v.parse::<u8>().unwrap_or(1))
        .unwrap_or(1);
    let require_mainnet_approval = policy
        .config
        .get("require_mainnet_approval")
        .map(|v| v == "true")
        .unwrap_or(true);

    let passed = if network == "mainnet" {
        if require_mainnet_approval {
            min_approvers >= 1
        } else {
            true
        }
    } else {
        true
    };

    ComplianceCheckResult {
        policy_id: policy.id.clone(),
        policy_name: policy.name.clone(),
        passed,
        severity: if !passed {
            ComplianceSeverity::Blocking
        } else {
            ComplianceSeverity::Info
        },
        message: if passed {
            format!("Approval requirements met for {} network", network)
        } else {
            format!(
                "Mainnet deployments require at least {} approver(s)",
                min_approvers
            )
        },
    }
}

fn check_deployment_window(policy: &CompliancePolicy) -> ComplianceCheckResult {
    let start_hour = policy
        .config
        .get("start_hour")
        .and_then(|v| v.parse::<u8>().ok())
        .unwrap_or(9);
    let end_hour = policy
        .config
        .get("end_hour")
        .and_then(|v| v.parse::<u8>().ok())
        .unwrap_or(17);
    let timezone_offset = policy
        .config
        .get("timezone_offset")
        .map(|v| v.parse::<i32>().ok())
        .flatten()
        .unwrap_or(0);

    let now = Utc::now();
    let hour = (now.hour() as i32 + timezone_offset).rem_euclid(24) as u8;

    let within_window = hour >= start_hour && hour < end_hour;

    ComplianceCheckResult {
        policy_id: policy.id.clone(),
        policy_name: policy.name.clone(),
        passed: within_window,
        severity: ComplianceSeverity::Warning,
        message: if within_window {
            format!(
                "Current time ({:02}:00 UTC{:+}) is within deployment window ({:02}:00-{:02}:00)",
                now.hour(),
                timezone_offset,
                start_hour,
                end_hour
            )
        } else {
            format!(
                "Current time ({:02}:00 UTC{:+}) is outside deployment window ({:02}:00-{:02}:00)",
                now.hour(),
                timezone_offset,
                start_hour,
                end_hour
            )
        },
    }
}

fn check_deployment_frequency(
    policy: &CompliancePolicy,
    network: &str,
    _requested_by: &str,
) -> ComplianceCheckResult {
    let max_per_hour = policy
        .config
        .get("max_per_hour")
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(5);

    let reports = load_reports_raw().unwrap_or_default();
    let one_hour_ago = Utc::now() - chrono::Duration::hours(1);
    let recent_count = reports
        .iter()
        .filter(|r| r.network == network)
        .filter(|r| {
            chrono::DateTime::parse_from_rfc3339(&r.timestamp)
                .ok()
                .map(|dt| chrono::DateTime::<Utc>::from(dt) > one_hour_ago)
                .unwrap_or(false)
        })
        .count();

    let passed = recent_count < max_per_hour;

    ComplianceCheckResult {
        policy_id: policy.id.clone(),
        policy_name: policy.name.clone(),
        passed,
        severity: ComplianceSeverity::Warning,
        message: if passed {
            format!(
                "Deployment frequency ({}/hour) is within limit ({}/hour)",
                recent_count, max_per_hour
            )
        } else {
            format!(
                "Deployment frequency ({}/hour) exceeds limit ({}/hour)",
                recent_count, max_per_hour
            )
        },
    }
}

fn check_network_restriction(policy: &CompliancePolicy, network: &str) -> ComplianceCheckResult {
    let allowed = policy.config.get("allowed_networks").map(|v| {
        v.split(',')
            .map(|s| s.trim().to_string())
            .collect::<Vec<_>>()
    });
    let blocked = policy.config.get("blocked_networks").map(|v| {
        v.split(',')
            .map(|s| s.trim().to_string())
            .collect::<Vec<_>>()
    });

    let mut passed = true;
    let mut message = format!("Network '{}' is allowed", network);

    if let Some(ref allowed_nets) = allowed {
        if !allowed_nets.contains(&network.to_string()) && !allowed_nets.contains(&"*".to_string())
        {
            passed = false;
            message = format!(
                "Network '{}' is not in the allowed list: {:?}",
                network, allowed_nets
            );
        }
    }

    if let Some(ref blocked_nets) = blocked {
        if blocked_nets.contains(&network.to_string()) {
            passed = false;
            message = format!("Network '{}' is in the blocked list", network);
        }
    }

    ComplianceCheckResult {
        policy_id: policy.id.clone(),
        policy_name: policy.name.clone(),
        passed,
        severity: if !passed {
            ComplianceSeverity::Blocking
        } else {
            ComplianceSeverity::Info
        },
        message,
    }
}

fn check_freeze_period(policy: &CompliancePolicy) -> ComplianceCheckResult {
    let freeze_start = policy.config.get("freeze_start");
    let freeze_end = policy.config.get("freeze_end");

    let now = Utc::now();

    if let (Some(start_str), Some(end_str)) = (freeze_start, freeze_end) {
        if let (Ok(start), Ok(end)) = (
            chrono::NaiveDateTime::parse_from_str(start_str, "%Y-%m-%dT%H:%M:%S"),
            chrono::NaiveDateTime::parse_from_str(end_str, "%Y-%m-%dT%H:%M:%S"),
        ) {
            let start_dt: DateTime<Utc> = DateTime::from_naive_utc_and_offset(start, Utc);
            let end_dt: DateTime<Utc> = DateTime::from_naive_utc_and_offset(end, Utc);

            let in_freeze = now >= start_dt && now <= end_dt;
            return ComplianceCheckResult {
                policy_id: policy.id.clone(),
                policy_name: policy.name.clone(),
                passed: !in_freeze,
                severity: ComplianceSeverity::Blocking,
                message: if in_freeze {
                    format!(
                        "Currently in deployment freeze period ({} to {})",
                        start_str, end_str
                    )
                } else {
                    "Not in a deployment freeze period".to_string()
                },
            };
        }
    }

    ComplianceCheckResult {
        policy_id: policy.id.clone(),
        policy_name: policy.name.clone(),
        passed: true,
        severity: ComplianceSeverity::Info,
        message: "No freeze period configured".to_string(),
    }
}

pub fn build_default_policies() -> Result<Vec<CompliancePolicy>> {
    let existing = load_policies_raw()?;
    if !existing.is_empty() {
        return Ok(existing);
    }

    let mut created = vec![];

    let p1 = create_policy(
        "Mainnet Approval Required",
        "Production deployments require at least one approval",
        PolicyType::RequiredApprovers,
        ComplianceSeverity::Blocking,
        [
            ("min_approvers".to_string(), "1".to_string()),
            ("require_mainnet_approval".to_string(), "true".to_string()),
        ]
        .into_iter()
        .collect(),
    )?;
    created.push(p1);

    let p2 = create_policy(
        "Deployment Window",
        "Deployments should occur during business hours (09:00-17:00 UTC)",
        PolicyType::DeploymentWindow,
        ComplianceSeverity::Warning,
        [
            ("start_hour".to_string(), "9".to_string()),
            ("end_hour".to_string(), "17".to_string()),
            ("timezone_offset".to_string(), "0".to_string()),
        ]
        .into_iter()
        .collect(),
    )?;
    created.push(p2);

    let p3 = create_policy(
        "Deployment Frequency Limit",
        "Maximum 5 deployments per hour per network",
        PolicyType::MaxDeploymentFrequency,
        ComplianceSeverity::Warning,
        [("max_per_hour".to_string(), "5".to_string())]
            .into_iter()
            .collect(),
    )?;
    created.push(p3);

    let p4 = create_policy(
        "Network Restriction",
        "Only testnet and mainnet are allowed for deployments",
        PolicyType::NetworkRestriction,
        ComplianceSeverity::Blocking,
        [(
            "allowed_networks".to_string(),
            "testnet,mainnet".to_string(),
        )]
        .into_iter()
        .collect(),
    )?;
    created.push(p4);

    Ok(created)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compliance_severity_display() {
        assert_eq!(ComplianceSeverity::Info.to_string(), "info");
        assert_eq!(ComplianceSeverity::Warning.to_string(), "warning");
        assert_eq!(ComplianceSeverity::Blocking.to_string(), "blocking");
    }

    #[test]
    fn test_network_restriction_allowed() {
        let policy = CompliancePolicy {
            id: "pol-1".to_string(),
            name: "Test".to_string(),
            description: "".to_string(),
            policy_type: PolicyType::NetworkRestriction,
            severity: ComplianceSeverity::Blocking,
            enabled: true,
            config: [(
                "allowed_networks".to_string(),
                "testnet,mainnet".to_string(),
            )]
            .into_iter()
            .collect(),
            created_at: "".to_string(),
            updated_at: "".to_string(),
        };
        let result = check_network_restriction(&policy, "testnet");
        assert!(result.passed);
    }

    #[test]
    fn test_network_restriction_blocked() {
        let policy = CompliancePolicy {
            id: "pol-2".to_string(),
            name: "Test".to_string(),
            description: "".to_string(),
            policy_type: PolicyType::NetworkRestriction,
            severity: ComplianceSeverity::Blocking,
            enabled: true,
            config: [("allowed_networks".to_string(), "testnet".to_string())]
                .into_iter()
                .collect(),
            created_at: "".to_string(),
            updated_at: "".to_string(),
        };
        let result = check_network_restriction(&policy, "mainnet");
        assert!(!result.passed);
    }
}
