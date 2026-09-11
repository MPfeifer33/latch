use serde::Serialize;
use serde_json::json;
use std::path::Path;

use crate::LatchError;

pub fn success_message(msg: &str, path: &Path, is_json: bool) {
    if is_json {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "ok": true,
                "message": msg,
                "path": path.display().to_string(),
            }))
            .unwrap()
        );
    } else {
        println!("{msg} at {}", path.display());
    }
}

pub fn print_json<T: Serialize>(value: &T) -> Result<(), LatchError> {
    Ok(agent_tools_core::print_raw_json(value)?)
}

pub fn print_json_value(value: serde_json::Value) -> Result<(), LatchError> {
    print_json(&value)
}
