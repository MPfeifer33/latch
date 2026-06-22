//! Integration tests for decisions, contracts, status, and prompt context.

use std::path::Path;
use std::process::{Command, Output};
use tempfile::TempDir;

fn latch(dir: &Path, actor: &str) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_latch"));
    cmd.arg("--repo").arg(dir);
    cmd.arg("--actor").arg(actor);
    cmd
}

fn init_workspace(dir: &Path) {
    let output = latch(dir, "test-agent").arg("init").output().unwrap();
    assert_success(&output, "init");
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
fn decision_lifecycle_and_contract_get_are_persisted() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();
    init_workspace(dir);

    let decision = json_output(
        latch(dir, "bjarn")
            .args([
                "decision",
                "add",
                "--title",
                "Use append-only coordination ledger",
                "--body",
                "Store decisions as events plus materialized views.",
                "--tag",
                "architecture",
                "--participant",
                "bjarn",
                "--participant",
                "nix",
            ])
            .output()
            .unwrap(),
        "decision add",
    );
    let decision_id = decision["decision"]["id"].as_str().unwrap().to_string();
    assert_eq!(decision["decision"]["status"], "active");

    let shown = json_output(
        latch(dir, "nix")
            .args(["decision", "show", &decision_id])
            .output()
            .unwrap(),
        "decision show",
    );
    assert_eq!(
        shown["decision"]["body"],
        "Store decisions as events plus materialized views."
    );
    assert_eq!(shown["decision"]["participants"][0], "bjarn");
    assert_eq!(shown["decision"]["tags"][0], "architecture");

    let superseded = json_output(
        latch(dir, "bjarn")
            .args([
                "decision",
                "supersede",
                &decision_id,
                "--title",
                "Use append-only coordination ledger v2",
                "--body",
                "Keep the event log canonical and derive compact views.",
            ])
            .output()
            .unwrap(),
        "decision supersede",
    );
    let new_decision_id = superseded["decision"]["id"].as_str().unwrap().to_string();

    let old = json_output(
        latch(dir, "nix")
            .args(["decision", "show", &decision_id])
            .output()
            .unwrap(),
        "decision show old",
    );
    assert_eq!(old["decision"]["status"], "superseded");
    assert_eq!(old["decision"]["superseded_by"], new_decision_id);

    let contract = json_output(
        latch(dir, "nix")
            .args([
                "contract",
                "set",
                "validation-result",
                "v1",
                "--body",
                r#"{"success":true,"diagnostics":[]}"#,
                "--owner",
                "nix",
                "--consumer",
                "bjarn",
            ])
            .output()
            .unwrap(),
        "contract set",
    );
    assert_eq!(contract["contract"]["name"], "validation-result");
    assert_eq!(contract["contract"]["consumers"][0], "bjarn");

    let fetched = json_output(
        latch(dir, "bjarn")
            .args(["contract", "get", "validation-result", "v1"])
            .output()
            .unwrap(),
        "contract get",
    );
    assert_eq!(fetched["contract"]["body"]["success"], true);
    assert_eq!(fetched["contract"]["owner"], "nix");
}

#[test]
fn status_json_and_context_text_summarize_coordination_state() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();
    init_workspace(dir);

    assert_success(
        &latch(dir, "bjarn")
            .args([
                "claim",
                "acquire",
                "frontend/",
                "--intent",
                "UI polish",
                "--ttl",
                "1h",
            ])
            .output()
            .unwrap(),
        "claim acquire",
    );
    assert_success(
        &latch(dir, "nix")
            .args([
                "task",
                "add",
                "--to",
                "bjarn",
                "--title",
                "Review context contract",
                "--priority",
                "high",
            ])
            .output()
            .unwrap(),
        "task add",
    );
    assert_success(
        &latch(dir, "bjarn")
            .args([
                "decision",
                "add",
                "--title",
                "Context defaults to prompt text",
                "--body",
                "JSON remains opt-in for context because prompts are the first consumer.",
            ])
            .output()
            .unwrap(),
        "decision add",
    );
    assert_success(
        &latch(dir, "nix")
            .args([
                "contract",
                "set",
                "context-output",
                "v1",
                "--body",
                r#"{"default":"text","json_flag":"--format json"}"#,
                "--consumer",
                "bjarn",
            ])
            .output()
            .unwrap(),
        "contract set",
    );
    assert_success(
        &latch(dir, "nix")
            .args([
                "note",
                "add",
                "--kind",
                "hazard",
                "--body",
                "Do not overwrite another agent's active claim.",
            ])
            .output()
            .unwrap(),
        "note add",
    );

    let status = json_output(
        latch(dir, "bjarn")
            .args(["--format", "json", "status", "--for", "bjarn"])
            .output()
            .unwrap(),
        "status json",
    );
    assert_eq!(status["status"]["actor"], "bjarn");
    assert_eq!(status["status"]["active_claims"][0]["path"], "frontend/");
    assert_eq!(
        status["status"]["tasks"][0]["title"],
        "Review context contract"
    );
    assert_eq!(
        status["status"]["recent_decisions"][0]["title"],
        "Context defaults to prompt text"
    );
    assert_eq!(status["status"]["contracts"][0]["name"], "context-output");
    assert_eq!(
        status["status"]["hazards"][0]["body"],
        "Do not overwrite another agent's active claim."
    );

    let context = latch(dir, "bjarn")
        .args(["context", "--for", "bjarn"])
        .output()
        .unwrap();
    assert_success(&context, "context text");
    let context_text = String::from_utf8_lossy(&context.stdout);
    assert!(context_text.starts_with("Latch context for bjarn"));
    assert!(context_text.contains("frontend/"));
    assert!(context_text.contains("Review context contract"));
    assert!(context_text.contains("Context defaults to prompt text"));
    assert!(context_text.contains("context-output"));
    assert!(context_text.contains("Do not overwrite another agent's active claim."));
    assert!(
        serde_json::from_slice::<serde_json::Value>(&context.stdout).is_err(),
        "context without --format json should default to text"
    );

    let context_json = json_output(
        latch(dir, "bjarn")
            .args(["--format", "json", "context", "--for", "bjarn"])
            .output()
            .unwrap(),
        "context json",
    );
    assert_eq!(context_json["context"]["actor"], "bjarn");
    assert_eq!(
        context_json["context"]["assigned_tasks"][0]["title"],
        "Review context contract"
    );
}
