//! A claim conflict must produce exactly one error document on stderr.

use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

fn latch(dir: &Path, actor: &str) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_latch"));
    cmd.arg("--repo").arg(dir);
    cmd.arg("--actor").arg(actor);
    cmd
}

fn init_workspace(dir: &Path) {
    let output = latch(dir, "nix").arg("init").output().unwrap();
    assert!(output.status.success());
}

#[test]
fn claim_conflict_json_is_a_single_document_with_conflicts() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();
    init_workspace(dir);

    let first = latch(dir, "nix")
        .args(["claim", "acquire", "src/lib.rs", "--ttl", "1h"])
        .output()
        .unwrap();
    assert!(first.status.success());

    let second = latch(dir, "other-agent")
        .args(["--format", "json", "claim", "acquire", "src/lib.rs", "--ttl", "1h"])
        .output()
        .unwrap();
    assert_eq!(second.status.code(), Some(2));
    assert!(second.stdout.is_empty(), "conflict must not print to stdout");

    let stderr = String::from_utf8_lossy(&second.stderr);
    // from_str rejects trailing data, so a second document would fail here.
    let doc: serde_json::Value = serde_json::from_str(stderr.trim())
        .unwrap_or_else(|err| panic!("stderr is not one JSON document: {err}\n{stderr}"));
    assert_eq!(doc["ok"], false);
    assert_eq!(doc["error"]["code"], "claim_conflict");
    assert_eq!(doc["error"]["message"], "Path is already claimed");
    let conflicts = doc["error"]["conflicts"].as_array().unwrap();
    assert_eq!(conflicts.len(), 1);
    assert_eq!(conflicts[0]["owner"], "nix");
    assert_eq!(conflicts[0]["path"], "src/lib.rs");
}

#[test]
fn claim_conflict_text_is_a_single_line() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();
    init_workspace(dir);

    let first = latch(dir, "nix")
        .args(["claim", "acquire", "src/", "--ttl", "1h"])
        .output()
        .unwrap();
    assert!(first.status.success());

    let second = latch(dir, "other-agent")
        .args(["--format", "text", "claim", "acquire", "src/main.rs", "--ttl", "1h"])
        .output()
        .unwrap();
    assert_eq!(second.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&second.stderr);
    assert_eq!(stderr.trim(), "error: claim conflict on src/main.rs");
}
