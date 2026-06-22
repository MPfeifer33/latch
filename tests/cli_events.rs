//! Integration tests for events commands

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
fn events_list_shows_init_event() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();
    init_workspace(dir);

    let output = latch(dir).args(["events", "list", "--limit", "10"]).output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let events = json["events"].as_array().unwrap();
    assert!(!events.is_empty());
    assert_eq!(events[0]["kind"], "workspace.initialized");
}

#[test]
fn events_accumulate_from_operations() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();
    init_workspace(dir);

    // Perform some operations
    latch(dir).args(["claim", "acquire", "file.rs", "--ttl", "1h"]).output().unwrap();
    latch(dir).args(["note", "add", "--kind", "hazard", "--body", "watch out"]).output().unwrap();
    latch(dir).args(["task", "add", "--to", "bjarn", "--title", "do thing"]).output().unwrap();

    let output = latch(dir).args(["events", "list", "--limit", "50"]).output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let events = json["events"].as_array().unwrap();

    // init + claim.acquired + note.added + task.added = 4 events
    assert_eq!(events.len(), 4);

    let kinds: Vec<&str> = events.iter().map(|e| e["kind"].as_str().unwrap()).collect();
    assert!(kinds.contains(&"workspace.initialized"));
    assert!(kinds.contains(&"claim.acquired"));
    assert!(kinds.contains(&"note.added"));
    assert!(kinds.contains(&"task.added"));
}

#[test]
fn events_show_by_id() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();
    init_workspace(dir);

    // Get the init event ID
    let output = latch(dir).args(["events", "list", "--limit", "1"]).output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let event_id = json["events"][0]["id"].as_str().unwrap().to_string();

    // Show it
    let output = latch(dir).args(["events", "show", &event_id]).output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(json["ok"], true);
    assert_eq!(json["event"]["id"], event_id);
    assert_eq!(json["event"]["kind"], "workspace.initialized");
}

#[test]
fn events_show_nonexistent_fails() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();
    init_workspace(dir);

    let output = latch(dir).args(["events", "show", "nonexistent-id"]).output().unwrap();
    assert_eq!(output.status.code(), Some(3)); // not found
}
