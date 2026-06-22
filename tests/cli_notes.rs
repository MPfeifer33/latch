//! Integration tests for note commands

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
    assert!(output.status.success());
}

#[test]
fn note_add_and_list() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();
    init_workspace(dir);

    // Add a hazard note
    let output = latch(dir)
        .args(["note", "add", "--kind", "hazard", "--body", "cargo test mutates target artifacts"])
        .output().unwrap();
    assert!(output.status.success(), "note add failed: {}", String::from_utf8_lossy(&output.stderr));

    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(json["ok"], true);
    assert_eq!(json["note"]["kind"], "hazard");

    // Add an observation
    let output = latch(dir)
        .args(["note", "add", "--kind", "observation", "--body", "Build takes 3 minutes on first run"])
        .output().unwrap();
    assert!(output.status.success());

    // List all notes
    let output = latch(dir).args(["note", "list"]).output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(json["notes"].as_array().unwrap().len(), 2);

    // List filtered by kind
    let output = latch(dir).args(["note", "list", "--kind", "hazard"]).output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(json["notes"].as_array().unwrap().len(), 1);
}

#[test]
fn note_remove() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();
    init_workspace(dir);

    let output = latch(dir)
        .args(["note", "add", "--kind", "handoff", "--body", "Left off at claims module"])
        .output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let note_id = json["note"]["id"].as_str().unwrap().to_string();

    // Remove it
    let output = latch(dir).args(["note", "remove", &note_id]).output().unwrap();
    assert!(output.status.success());

    // List should be empty
    let output = latch(dir).args(["note", "list"]).output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(json["notes"].as_array().unwrap().len(), 0);
}

#[test]
fn note_invalid_kind_rejected() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();
    init_workspace(dir);

    let output = latch(dir)
        .args(["note", "add", "--kind", "invalid", "--body", "should fail"])
        .output().unwrap();
    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(1)); // validation error
}

#[test]
fn note_remove_nonexistent_fails() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();
    init_workspace(dir);

    let output = latch(dir).args(["note", "remove", "nonexistent-id"]).output().unwrap();
    assert_eq!(output.status.code(), Some(3)); // not found
}
