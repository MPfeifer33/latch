//! Integration tests for task commands

use std::process::Command;
use tempfile::TempDir;

fn latch(dir: &std::path::Path) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_latch"));
    cmd.arg("--repo").arg(dir);
    cmd.arg("--actor").arg("nix");
    cmd
}

fn init_workspace(dir: &std::path::Path) {
    let output = latch(dir).arg("init").output().unwrap();
    assert!(output.status.success());
}

#[test]
fn task_add_and_list() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();
    init_workspace(dir);

    // Add a task
    let output = latch(dir)
        .args(["task", "add", "--to", "bjarn", "--title", "Implement contracts module", "--priority", "high"])
        .output().unwrap();
    assert!(output.status.success(), "task add failed: {}", String::from_utf8_lossy(&output.stderr));

    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(json["ok"], true);
    assert_eq!(json["task"]["status"], "open");
    assert_eq!(json["task"]["assigned_to"], "bjarn");

    // List all tasks
    let output = latch(dir).args(["task", "list"]).output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(json["tasks"].as_array().unwrap().len(), 1);

    // List filtered by assignee
    let output = latch(dir).args(["task", "list", "--for", "bjarn"]).output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(json["tasks"].as_array().unwrap().len(), 1);

    // List for different actor (empty)
    let output = latch(dir).args(["task", "list", "--for", "nobody"]).output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(json["tasks"].as_array().unwrap().len(), 0);
}

#[test]
fn task_lifecycle_open_taken_done() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();
    init_workspace(dir);

    // Add
    let output = latch(dir)
        .args(["task", "add", "--to", "nix", "--title", "Write tests"])
        .output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let task_id = json["task"]["id"].as_str().unwrap().to_string();

    // Take
    let output = latch(dir).args(["task", "take", &task_id]).output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(json["status"], "taken");

    // Done
    let output = latch(dir).args(["task", "done", &task_id]).output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(json["status"], "done");

    // Should not appear in active list anymore
    let output = latch(dir).args(["task", "list"]).output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(json["tasks"].as_array().unwrap().len(), 0);
}

#[test]
fn task_cancel() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();
    init_workspace(dir);

    let output = latch(dir)
        .args(["task", "add", "--to", "bjarn", "--title", "Canceled task"])
        .output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let task_id = json["task"]["id"].as_str().unwrap().to_string();

    let output = latch(dir).args(["task", "cancel", &task_id]).output().unwrap();
    assert!(output.status.success());

    // Not in active list
    let output = latch(dir).args(["task", "list"]).output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(json["tasks"].as_array().unwrap().len(), 0);
}

#[test]
fn task_take_wrong_status_fails() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();
    init_workspace(dir);

    let output = latch(dir)
        .args(["task", "add", "--to", "nix", "--title", "Status test"])
        .output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let task_id = json["task"]["id"].as_str().unwrap().to_string();

    // Take it
    latch(dir).args(["task", "take", &task_id]).output().unwrap();

    // Try to take again (already taken, not open)
    let output = latch(dir).args(["task", "take", &task_id]).output().unwrap();
    assert_eq!(output.status.code(), Some(3)); // not found (wrong status)
}
