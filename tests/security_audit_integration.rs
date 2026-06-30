use starforge::utils::security::{
    format_html_report, format_report, generate_github_actions_workflow, run_audit, AuditConfig,
};
use std::io::Write;
use tempfile::NamedTempFile;

const INSECURE_CONTRACT: &str = r#"
#![no_std]
use soroban_sdk::{contract, contractimpl, Env};

#[contract]
pub struct Token;

#[contractimpl]
impl Token {
    pub fn transfer(env: Env, amount: u64) -> u64 {
        let balance: u64 = env.storage().instance().get(&()).unwrap();
        balance + amount
    }
}
"#;

fn write_contract() -> NamedTempFile {
    let mut file = NamedTempFile::new().unwrap();
    file.write_all(INSECURE_CONTRACT.as_bytes()).unwrap();
    file
}

#[test]
fn audit_runs_builtin_pipeline_and_records_tool_statuses() {
    let file = write_contract();
    let result = run_audit(
        file.path(),
        &AuditConfig {
            run_slither: false,
            run_mythril: false,
        },
    )
    .unwrap();

    assert!(result.tools_used.contains(&"starforge-builtin".to_string()));
    assert!(result
        .tool_statuses
        .iter()
        .any(|tool| tool.tool == "starforge-builtin" && tool.executed));
    assert!(result
        .tool_statuses
        .iter()
        .any(|tool| tool.tool == "slither" && tool.status == "skipped"));
    assert!(!result.findings.is_empty());
    assert!(result.score < 100.0);
}

#[test]
fn audit_reports_include_remediation_and_tool_status() {
    let file = write_contract();
    let result = run_audit(
        file.path(),
        &AuditConfig {
            run_slither: false,
            run_mythril: false,
        },
    )
    .unwrap();

    let text = format_report(&result);
    assert!(text.contains("Tool Status"));
    assert!(text.contains("Remediation"));

    let html = format_html_report(&result);
    assert!(html.contains("<table"));
    assert!(html.contains("Security Audit Report"));
}

#[test]
fn ci_workflow_generation_invokes_security_audit() {
    let workflow =
        generate_github_actions_workflow(std::path::Path::new("contracts/token/src/lib.rs"), 85.0);

    assert!(workflow.contains("StarForge Security Audit"));
    assert!(workflow.contains("security audit contracts/token/src/lib.rs"));
    assert!(workflow.contains("--min-score 85.0"));
}
