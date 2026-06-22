//! Integration tests for claim commands

use std::process::Command;
use tempfile::TempDir;

fn latch(dir: &std::path::Path) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_latch"));
    cmd.arg("--repo").arg(dir);
    cmd.arg("--actor").arg("test-agent");
    cmd
}

fn init_workspace(dir: &std::path::Path) {
    let output = latch(dir).arg("init").output().unwrap();
    assert!(output.status.success(), "init failed: {}", String::from_utf8_lossy(&output.stderr));
}

#[test]
fn init_creates_workspace_and_is_idempotent() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();

    // First init
    let output = latch(dir).arg("init").output().unwrap();
    assert!(output.status.success());
    assert!(dir.join(".agent-workspace/workspace.sqlite").exists());

    // Second init (idempotent)
    let output = latch(dir).arg("init").output().unwrap();
    assert!(output.status.success());
}

#[test]
fn claim_acquire_and_list() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();
    init_workspace(dir);

    // Acquire a claim
    let output = latch(dir)
        .args(["claim", "acquire", "src/main.rs", "--intent", "refactoring", "--ttl", "1h"])
        .output().unwrap();
    assert!(output.status.success(), "acquire failed: {}", String::from_utf8_lossy(&output.stderr));

    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(json["ok"], true);
    assert_eq!(json["claim"]["path"], "src/main.rs");
    assert_eq!(json["claim"]["owner"], "test-agent");

    // List claims
    let output = latch(dir).args(["claim", "list"]).output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(json["claims"].as_array().unwrap().len(), 1);
}

#[test]
fn claim_conflict_on_same_file() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();
    init_workspace(dir);

    // First claim
    let output = latch(dir)
        .args(["claim", "acquire", "src/lib.rs", "--ttl", "2h"])
        .output().unwrap();
    assert!(output.status.success());

    // Second claim on same file (different actor)
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_latch"));
    cmd.arg("--repo").arg(dir);
    cmd.arg("--actor").arg("other-agent");
    let output = cmd.args(["claim", "acquire", "src/lib.rs", "--ttl", "1h"]).output().unwrap();

    // Should fail with exit code 2 (claim conflict)
    assert_eq!(output.status.code(), Some(2));
}

#[test]
fn claim_directory_conflicts_with_child_file() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();
    init_workspace(dir);

    // Claim a directory
    let output = latch(dir)
        .args(["claim", "acquire", "src/", "--intent", "restructure", "--ttl", "2h"])
        .output().unwrap();
    assert!(output.status.success());

    // Claim a file inside that directory (different actor)
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_latch"));
    cmd.arg("--repo").arg(dir);
    cmd.arg("--actor").arg("other-agent");
    let output = cmd.args(["claim", "acquire", "src/main.rs", "--ttl", "1h"]).output().unwrap();

    assert_eq!(output.status.code(), Some(2));
}

#[test]
fn claim_release_allows_reacquire() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();
    init_workspace(dir);

    // Acquire
    let output = latch(dir)
        .args(["claim", "acquire", "README.md", "--ttl", "1h"])
        .output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let claim_id = json["claim"]["id"].as_str().unwrap().to_string();

    // Release
    let output = latch(dir)
        .args(["claim", "release", &claim_id])
        .output().unwrap();
    assert!(output.status.success());

    // Re-acquire by different actor should succeed
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_latch"));
    cmd.arg("--repo").arg(dir);
    cmd.arg("--actor").arg("other-agent");
    let output = cmd.args(["claim", "acquire", "README.md", "--ttl", "1h"]).output().unwrap();
    assert!(output.status.success());
}

#[test]
fn claim_renew_extends_ttl() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();
    init_workspace(dir);

    // Acquire with short TTL
    let output = latch(dir)
        .args(["claim", "acquire", "Cargo.toml", "--ttl", "30m"])
        .output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let claim_id = json["claim"]["id"].as_str().unwrap().to_string();
    let original_expires = json["claim"]["expires_at"].as_str().unwrap().to_string();

    // Renew with longer TTL
    let output = latch(dir)
        .args(["claim", "renew", &claim_id, "--ttl", "4h"])
        .output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let new_expires = json["expires_at"].as_str().unwrap();

    assert_ne!(original_expires, new_expires);
}

#[test]
fn claim_path_traversal_rejected() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();
    init_workspace(dir);

    let output = latch(dir)
        .args(["claim", "acquire", "../etc/passwd", "--ttl", "1h"])
        .output().unwrap();
    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(1)); // validation error
}
