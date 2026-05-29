//! Fast CLI smoke tests for core user paths (no network required).

use std::process::Command;

fn starforge() -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_starforge"));
    cmd.arg("-q");
    cmd
}

fn assert_success(output: &std::process::Output, cmd: &str) {
    assert!(
        output.status.success(),
        "{} failed: {}",
        cmd,
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn info_exits_zero() {
    let output = starforge().arg("info").output().expect("spawn info");
    assert_success(&output, "starforge info");
}

#[test]
fn version_prints_release() {
    let output = starforge().arg("--version").output().expect("spawn version");
    assert_success(&output, "starforge --version");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("starforge"));
}

#[test]
fn help_lists_wallet_command() {
    let output = starforge().arg("--help").output().expect("spawn help");
    assert_success(&output, "starforge --help");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("wallet"));
}

#[test]
fn network_show_exits_zero() {
    let output = starforge()
        .args(["network", "show"])
        .output()
        .expect("spawn network show");
    assert_success(&output, "starforge network show");
}

#[test]
fn wallet_list_exits_zero() {
    let output = starforge()
        .args(["wallet", "list"])
        .output()
        .expect("spawn wallet list");
    assert_success(&output, "starforge wallet list");
}

#[test]
fn template_list_exits_zero() {
    let output = starforge()
        .args(["template", "list"])
        .output()
        .expect("spawn template list");
    assert_success(&output, "starforge template list");
}

#[test]
fn deploy_help_documents_flags() {
    let output = starforge()
        .args(["deploy", "--help"])
        .output()
        .expect("spawn deploy help");
    assert_success(&output, "starforge deploy --help");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("--simulate"));
    assert!(stdout.contains("--execute"));
}
