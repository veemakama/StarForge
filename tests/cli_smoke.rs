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

#[test]
fn network_add_custom_succeeds() {
    let output = starforge()
        .args([
            "network",
            "add",
            "my-smoke-test-net",
            "--horizon-url",
            "https://example.com/horizon",
            "--soroban-rpc-url",
            "https://example.com/rpc",
        ])
        .output()
        .expect("spawn network add");
    assert_success(&output, "starforge network add");

    // Verify it appears in the list
    let list_output = starforge()
        .args(["network", "show"])
        .output()
        .expect("spawn network show");
    assert_success(&list_output, "starforge network show");
    let stdout = String::from_utf8_lossy(&list_output.stdout);
    assert!(
        stdout.to_lowercase().contains("my-smoke-test-net"),
        "expected 'my-smoke-test-net' in network show output"
    );
}

#[test]
fn network_add_rejects_empty_horizon_url() {
    let output = starforge()
        .args(["network", "add", "bad-net", "--horizon-url", ""])
        .output()
        .expect("spawn network add with empty url");
    assert!(
        !output.status.success(),
        "expected non-zero exit for empty horizon URL"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("cannot be empty") || stderr.contains("must start with"),
        "expected URL validation message in stderr, got: {}",
        stderr
    );
}

#[test]
fn network_switch_to_mainnet_succeeds() {
    let output = starforge()
        .args(["network", "switch", "mainnet"])
        .output()
        .expect("spawn network switch mainnet");
    assert_success(&output, "starforge network switch mainnet");
}

#[test]
fn network_switch_unknown_network_fails() {
    let output = starforge()
        .args(["network", "switch", "does-not-exist-xyz"])
        .output()
        .expect("spawn network switch unknown");
    assert!(
        !output.status.success(),
        "expected non-zero exit for unknown network"
    );
}

#[test]
fn network_add_reserved_name_fails() {
    let output = starforge()
        .args([
            "network",
            "add",
            "testnet",
            "--horizon-url",
            "https://example.com",
        ])
        .output()
        .expect("spawn network add testnet");
    assert!(
        !output.status.success(),
        "expected non-zero exit when overwriting reserved network name"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("reserved"),
        "expected 'reserved' in stderr, got: {}",
        stderr
    );
}
