//! `--version` must print `<name> <version>` (shared contract across the agent tools).

use std::process::Command;

#[test]
fn version_flag_prints_name_and_version() {
    let output = Command::new(env!("CARGO_BIN_EXE_latch"))
        .arg("--version")
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout.trim(),
        format!("latch {}", env!("CARGO_PKG_VERSION"))
    );
}
