//! End-to-end tests that drive the real `dk` binary through its public
//! interface: argv in, (stdout, stderr, exit code) out.
//!
//! No agent is ever invoked. Every case exercises a meta command (`--version`,
//! `--help`) or an error path that fails fast — before the review pipeline
//! builds an agent runner — so the suite is deterministic and offline. Each run
//! happens in a fresh temp dir so it can never pick up the repository's own
//! `dk.toml` or installed packs.

use std::process::{Command, Output};

fn run_in_empty_dir(args: &[&str]) -> Output {
    let tmp = tempfile::tempdir().unwrap();
    Command::new(env!("CARGO_BIN_EXE_dk"))
        .args(args)
        .current_dir(tmp.path())
        .output()
        .expect("failed to spawn dk binary")
}

#[test]
fn version_flag_prints_version_and_exits_zero() {
    let out = run_in_empty_dir(&["--version"]);
    assert!(out.status.success(), "exit status: {:?}", out.status);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains(env!("CARGO_PKG_VERSION")),
        "version output should contain the crate version, got: {stdout:?}"
    );
}

#[test]
fn help_lists_the_core_commands() {
    let out = run_in_empty_dir(&["--help"]);
    assert!(out.status.success(), "exit status: {:?}", out.status);
    let stdout = String::from_utf8_lossy(&out.stdout);
    for cmd in ["review", "check", "init", "packs"] {
        assert!(
            stdout.contains(cmd),
            "help text should list `{cmd}`, got: {stdout}"
        );
    }
}

#[test]
fn unknown_command_exits_two() {
    let out = run_in_empty_dir(&["definitely-not-a-command"]);
    assert_eq!(
        out.status.code(),
        Some(2),
        "unknown command should be a usage error (exit 2); stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn review_without_template_fails_with_validation_error() {
    let out = run_in_empty_dir(&["review"]);
    assert_eq!(
        out.status.code(),
        Some(1),
        "missing --template should exit 1"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("DK_INPUT_VALIDATION"),
        "stderr should carry the error code, got: {stderr}"
    );
    assert!(
        stderr.contains("--template"),
        "error should name the missing flag, got: {stderr}"
    );
}

#[test]
fn review_with_unknown_template_fails_before_invoking_an_agent() {
    let out = run_in_empty_dir(&["review", "--template", "no-such-pack-xyz"]);
    assert_eq!(out.status.code(), Some(1), "unresolved pack should exit 1");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("error ["),
        "stderr should be a coded `dk` error, got: {stderr}"
    );
}

#[test]
fn mcp_install_dry_run_exits_zero_and_prints_config() {
    let out = run_in_empty_dir(&[
        "mcp",
        "install",
        "--agent",
        "cursor",
        "--stdio",
        "--dry-run",
    ]);
    assert!(
        out.status.success(),
        "dry-run should exit 0; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("mcp") && stdout.contains("serve") && stdout.contains("stdio"),
        "dry-run output should contain mcp serve --transport stdio args, got: {stdout}"
    );
}

#[test]
fn mcp_help_lists_install_register_list_serve() {
    let out = run_in_empty_dir(&["mcp", "--help"]);
    assert!(
        out.status.success(),
        "mcp --help should exit 0; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    for cmd in ["install", "register", "list", "serve"] {
        assert!(
            stdout.contains(cmd),
            "mcp --help should list `{cmd}`, got: {stdout}"
        );
    }
}

#[test]
fn mcp_list_exits_zero() {
    let out = run_in_empty_dir(&["mcp", "list"]);
    assert!(
        out.status.success(),
        "mcp list should exit 0; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}
