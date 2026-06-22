use std::path::Path;
use serde_json::json;

pub fn success_message(msg: &str, path: &Path, is_json: bool) {
    if is_json {
        println!("{}", serde_json::to_string_pretty(&json!({
            "ok": true,
            "message": msg,
            "path": path.display().to_string(),
        })).unwrap());
    } else {
        println!("{msg} at {}", path.display());
    }
}
