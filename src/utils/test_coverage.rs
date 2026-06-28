use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CoverageReport {
    pub functions_total: u32,
    pub functions_covered: u32,
    pub lines_total: u32,
    pub lines_covered: u32,
    pub branches_total: u32,
    pub branches_covered: u32,
    pub uncovered_functions: Vec<String>,
    pub coverage_percent: f64,
}

pub fn analyze_source_coverage(source: &str, executed_functions: &[String]) -> CoverageReport {
    let all_functions: Vec<String> = source
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            if trimmed.starts_with("pub fn ") {
                trimmed
                    .strip_prefix("pub fn ")
                    .and_then(|rest| rest.split('(').next())
                    .map(|s| s.trim().to_string())
            } else {
                None
            }
        })
        .collect();

    let executed: HashSet<_> = executed_functions.iter().cloned().collect();
    let uncovered: Vec<String> = all_functions
        .iter()
        .filter(|f| !executed.contains(*f))
        .cloned()
        .collect();

    let functions_total = all_functions.len() as u32;
    let functions_covered = functions_total - uncovered.len() as u32;
    let lines_total = source.lines().count() as u32;
    let lines_covered = estimate_lines_covered(source, &executed);
    let branches_total = count_branches(source);
    let branches_covered = (branches_total as f64 * 0.7) as u32; // heuristic

    let coverage_percent = if functions_total == 0 {
        100.0
    } else {
        (functions_covered as f64 / functions_total as f64) * 100.0
    };

    CoverageReport {
        functions_total,
        functions_covered,
        lines_total,
        lines_covered,
        branches_total,
        branches_covered,
        uncovered_functions: uncovered,
        coverage_percent,
    }
}

fn estimate_lines_covered(source: &str, executed: &HashSet<String>) -> u32 {
    let mut covered = 0u32;
    let mut current_fn: Option<String> = None;
    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("pub fn ") {
            current_fn = trimmed
                .strip_prefix("pub fn ")
                .and_then(|rest| rest.split('(').next())
                .map(|s| s.trim().to_string());
        }
        if current_fn
            .as_ref()
            .is_some_and(|f| executed.contains(f) && !trimmed.is_empty() && !trimmed.starts_with("//"))
        {
            covered += 1;
        }
    }
    covered
}

fn count_branches(source: &str) -> u32 {
    source
        .lines()
        .filter(|l| {
            let t = l.trim();
            t.starts_with("if ") || t.contains(" match ") || t.starts_with("match ")
        })
        .count() as u32
}
