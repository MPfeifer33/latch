use std::path::Path;
use rusqlite::Connection;

use crate::LatchError;

pub fn show_status(_conn: &Connection, _actor: Option<&str>, _is_json: bool) -> Result<(), LatchError> {
    // Stub: Bjarn will implement status aggregator
    Err(LatchError::Validation("status not yet implemented".into()))
}

pub fn show_context(_conn: &Connection, _repo: &Path, _actor: Option<&str>, _is_json: bool) -> Result<(), LatchError> {
    // Stub: Bjarn will implement context aggregator
    Err(LatchError::Validation("context not yet implemented".into()))
}
