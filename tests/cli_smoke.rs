//! Fast CLI smoke tests for core user paths (no network required).

use std::process::Command;

fn isolated_home() -> tempfile::TempDir {
    tempfile::tempdir().expect("create isolated home")
}

fn starforge(home: &std::path::Path) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_starforge"));
    cmd.arg("-q");
    cmd.env("HOME", home);
    cmd.env("USERPROFILE", home);
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
    let home = isolated_home();
    let output = starforge(home.path())
        .arg("info")
        .output()
        .expect("spawn info");
    assert_success(&output, "starforge info");
}

#[test]
fn version_prints_release() {
    let home = isolated_home();
    let output = starforge(home.path())
        .arg("--version")
        .output()
        .expect("spawn version");
    assert_success(&output, "starforge --version");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("starforge"));
}

#[test]
fn help_lists_wallet_command() {
    let home = isolated_home();
    let output = starforge(home.path())
        .arg("--help")
        .output()
        .expect("spawn help");
    assert_success(&output, "starforge --help");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("wallet"));
}

#[test]
fn network_show_exits_zero() {
    let home = isolated_home();
    let output = starforge(home.path())
        .args(["network", "show"])
        .output()
        .expect("spawn network show");
    assert_success(&output, "starforge network show");
}

#[test]
fn wallet_list_exits_zero() {
    let home = isolated_home();
    let output = starforge(home.path())
        .args(["wallet", "list"])
        .output()
        .expect("spawn wallet list");
    assert_success(&output, "starforge wallet list");
}

#[test]
fn template_list_exits_zero() {
    let home = isolated_home();
    let output = starforge(home.path())
        .args(["template", "list"])
        .output()
        .expect("spawn template list");
    assert_success(&output, "starforge template list");
}

#[test]
fn deploy_help_documents_flags() {
    let home = isolated_home();
    let output = starforge(home.path())
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
    let home = isolated_home();
    let output = starforge(home.path())
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
    let list_output = starforge(home.path())
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
    let home = isolated_home();
    let output = starforge(home.path())
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
    let home = isolated_home();
    let output = starforge(home.path())
        .args(["network", "switch", "mainnet"])
        .output()
        .expect("spawn network switch mainnet");
    assert_success(&output, "starforge network switch mainnet");
}

#[test]
fn network_switch_unknown_network_fails() {
    let home = isolated_home();
    let output = starforge(home.path())
        .args(["network", "switch", "does-not-exist-xyz"])
        .output()
        .expect("spawn network switch unknown");
    assert!(
        !output.status.success(),
        "expected non-zero exit for unknown network"
    );
}

#[test]
fn network_remove_custom_succeeds() {
    let home = isolated_home();
    starforge(home.path())
        .args([
            "network",
            "add",
            "removable-net",
            "--horizon-url",
            "https://example.com/horizon",
        ])
        .output()
        .expect("spawn network add");

    let output = starforge(home.path())
        .args(["network", "remove", "removable-net"])
        .output()
        .expect("spawn network remove");
    assert_success(&output, "starforge network remove");

    let show = starforge(home.path())
        .args(["network", "show"])
        .output()
        .expect("spawn network show");
    let stdout = String::from_utf8_lossy(&show.stdout);
    assert!(
        !stdout.to_lowercase().contains("removable-net"),
        "removed network should not appear in show output"
    );
}

#[test]
fn network_remove_reserved_fails() {
    let home = isolated_home();
    let output = starforge(home.path())
        .args(["network", "remove", "testnet"])
        .output()
        .expect("spawn network remove testnet");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("built-in") || stderr.contains("cannot be removed"));
}

#[test]
fn network_rename_custom_succeeds() {
    let home = isolated_home();
    starforge(home.path())
        .args([
            "network",
            "add",
            "old-net",
            "--horizon-url",
            "https://example.com/horizon",
        ])
        .output()
        .expect("spawn network add");

    let output = starforge(home.path())
        .args(["network", "rename", "old-net", "new-net"])
        .output()
        .expect("spawn network rename");
    assert_success(&output, "starforge network rename");

    let show = starforge(home.path())
        .args(["network", "show"])
        .output()
        .expect("spawn network show");
    let stdout = String::from_utf8_lossy(&show.stdout);
    assert!(stdout.to_lowercase().contains("new-net"));
}

#[test]
fn network_add_reserved_name_fails() {
    let home = isolated_home();
    let output = starforge(home.path())
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
