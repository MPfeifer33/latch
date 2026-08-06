//! Integration tests for the agent-facing doctor command.

use std::path::Path;
use std::process::{Command, Output};
use tempfile::TempDir;

fn latch(dir: &Path, actor: &str) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_latch"));
    cmd.arg("--repo").arg(dir);
    cmd.arg("--actor").arg(actor);
    cmd
}

fn assert_success(output: &Output, label: &str) {
    assert!(
        output.status.success(),
        "{label} failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn json_output(output: Output, label: &str) -> serde_json::Value {
    assert_success(&output, label);
    serde_json::from_slice(&output.stdout).unwrap_or_else(|err| {
        panic!(
            "{label} returned invalid json: {err}\nstdout:\n{}",
            String::from_utf8_lossy(&output.stdout)
        )
    })
}

#[test]
fn doctor_reports_missing_workspace_without_creating_it() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();

    let report = json_output(
        latch(dir, "bjarn")
            .args(["doctor", "--format", "json"])
            .output()
            .unwrap(),
        "doctor",
    );

    assert_eq!(report["ok"], true);
    assert_eq!(report["schema_version"], "latch.doctor.v1");
    assert_eq!(report["action_level"], "initialize");
    assert_eq!(report["doctor"]["workspace"]["initialized"], false);
    assert_eq!(report["doctor"]["gates"]["workspace_initialized"], false);
    assert_eq!(
        report["doctor"]["recommended_commands"][0]["argv"],
        serde_json::json!(["latch", "init"])
    );
    assert!(!dir.join(".agent-workspace/workspace.sqlite").exists());
}

#[test]
fn doctor_surfaces_actor_tasks_and_workspace_hazards() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();

    assert_success(&latch(dir, "bjarn").arg("init").output().unwrap(), "init");
    assert_success(
        &latch(dir, "helix")
            .args([
                "task",
                "add",
                "--to",
                "bjarn",
                "--title",
                "Review Latch doctor output",
            ])
            .output()
            .unwrap(),
        "task add",
    );
    assert_success(
        &latch(dir, "helix")
            .args([
                "note",
                "add",
                "--kind",
                "hazard",
                "--body",
                "Do not skip coordination gates.",
            ])
            .output()
            .unwrap(),
        "hazard add",
    );
    assert_success(
        &latch(dir, "helix")
            .args([
                "contract",
                "set",
                "doctor-contract",
                "v1",
                "--body",
                r#"{"ok":true}"#,
                "--consumer",
                "bjarn",
            ])
            .output()
            .unwrap(),
        "contract set",
    );

    let report = json_output(
        latch(dir, "bjarn")
            .args(["doctor", "--for", "bjarn", "--format", "json"])
            .output()
            .unwrap(),
        "doctor",
    );

    assert_eq!(report["doctor"]["workspace"]["initialized"], true);
    assert_eq!(report["doctor"]["actor"], "bjarn");
    assert_eq!(report["doctor"]["action_level"], "review");
    assert_eq!(report["doctor"]["gates"]["assigned_tasks"], true);
    assert_eq!(report["doctor"]["gates"]["active_hazards"], true);
    assert_eq!(report["doctor"]["gates"]["active_contracts"], true);
    assert_eq!(report["doctor"]["counts"]["assigned_tasks"], 1);
    assert_eq!(report["doctor"]["counts"]["active_hazards"], 1);
    assert!(report["doctor"]["recommended_commands"]
        .as_array()
        .unwrap()
        .iter()
        .any(|command| command["reason_code"] == "active_hazards"));
    assert!(report["doctor"]["recommended_commands"]
        .as_array()
        .unwrap()
        .iter()
        .any(|command| command["reason_code"] == "active_contracts"));
}

#[test]
fn strict_doctor_uses_gate_exit_code_while_emitting_success_json() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();

    let output = latch(dir, "bjarn")
        .args(["doctor", "--strict", "--format", "json"])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(10));
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["ok"], true);
    assert_eq!(report["action_level"], "initialize");
}
