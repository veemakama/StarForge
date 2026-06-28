use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneratedTestCase {
    pub name: String,
    pub description: String,
    pub function: String,
    pub test_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestGenerationResult {
    pub source: String,
    pub cases: Vec<GeneratedTestCase>,
    pub output_path: Option<String>,
}

pub fn generate_from_source(source_path: &Path) -> Result<TestGenerationResult> {
    let content = fs::read_to_string(source_path)
        .with_context(|| format!("Failed to read {}", source_path.display()))?;
    let functions = extract_public_functions(&content);
    let source = source_path.to_string_lossy().to_string();

    let mut cases = Vec::new();
    for func in &functions {
        cases.push(GeneratedTestCase {
            name: format!("test_{}_happy_path", func),
            description: format!("Happy-path test for {}", func),
            function: func.clone(),
            test_type: "happy_path".into(),
        });
        cases.push(GeneratedTestCase {
            name: format!("test_{}_unauthorized", func),
            description: format!("Authorization failure test for {}", func),
            function: func.clone(),
            test_type: "auth_failure".into(),
        });
        if is_mutating(&content, func) {
            cases.push(GeneratedTestCase {
                name: format!("test_{}_zero_amount", func),
                description: format!("Zero/invalid input test for {}", func),
                function: func.clone(),
                test_type: "input_validation".into(),
            });
        }
    }

    Ok(TestGenerationResult {
        source,
        cases,
        output_path: None,
    })
}

pub fn write_generated_tests(result: &TestGenerationResult, output_path: &Path) -> Result<()> {
    let mut code = String::from("#[cfg(test)]\nmod generated_tests {\n");
    code.push_str("    use super::*;\n\n");

    for case in &result.cases {
        code.push_str(&format!("    /// {}\n", case.description));
        code.push_str(&format!("    #[test]\n    fn {}() {{\n", case.name));
        code.push_str("        let env = Env::default();\n");
        code.push_str("        // TODO: wire contract client and assert expected behavior\n");
        code.push_str(&format!(
            "        // Generated {} test for `{}`\n",
            case.test_type, case.function
        ));
        code.push_str("    }\n\n");
    }
    code.push_str("}\n");

    fs::write(output_path, code)
        .with_context(|| format!("Failed to write {}", output_path.display()))?;
    Ok(())
}

fn extract_public_functions(content: &str) -> Vec<String> {
    content
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
        .collect()
}

fn is_mutating(content: &str, func: &str) -> bool {
    let mut in_fn = false;
    for line in content.lines() {
        if line.contains(&format!("pub fn {}", func)) {
            in_fn = true;
        }
        if in_fn {
            if line.contains(".set(") || line.contains("transfer") || line.contains("mint") {
                return true;
            }
            if line.starts_with("pub fn ") && !line.contains(func) {
                break;
            }
        }
    }
    false
}
