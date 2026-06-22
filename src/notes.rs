use chrono::Utc;
use rusqlite::Connection;
use serde_json::json;
use ulid::Ulid;

use crate::cli::{Cli, NoteCommand};
use crate::db;
use crate::LatchError;

pub fn handle(cmd: &NoteCommand, cli: &Cli) -> Result<(), LatchError> {
    let repo = cli.resolve_repo()?;
    let conn = db::open_workspace(&repo)?;
    let actor = cli.resolve_actor();
    let repo_id = db::compute_repo_id(&repo);

    match cmd {
        NoteCommand::Add { kind, body } => add(&conn, &repo_id, &actor, kind, body, cli.is_json()),
        NoteCommand::List { kind } => list(&conn, kind.as_deref(), cli.is_json()),
        NoteCommand::Remove { id } => remove(&conn, &repo_id, &actor, id, cli.is_json()),
    }
}

fn add(conn: &Connection, repo_id: &str, actor: &str, kind: &str, body: &str, is_json: bool) -> Result<(), LatchError> {
    let valid_kinds = ["hazard", "handoff", "observation"];
    if !valid_kinds.contains(&kind) {
        return Err(LatchError::Validation(format!(
            "Invalid note kind: {kind}. Must be one of: {}",
            valid_kinds.join(", ")
        )));
    }

    let id = Ulid::new().to_string();
    let now = Utc::now().to_rfc3339();

    conn.execute(
        "INSERT INTO notes (id, kind, body, author, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![id, kind, body, actor, now],
    )?;

    db::append_event(conn, repo_id, actor, "note.added", "note", &id, None, json!({
        "kind": kind,
        "body": body,
    }))?;

    if is_json {
        println!("{}", serde_json::to_string_pretty(&json!({
            "ok": true,
            "note": { "id": id, "kind": kind }
        }))?);
    } else {
        println!("Note added: [{kind}] {id}");
    }
    Ok(())
}

fn list(conn: &Connection, kind: Option<&str>, is_json: bool) -> Result<(), LatchError> {
    let (query, params): (String, Vec<Box<dyn rusqlite::types::ToSql>>) = if let Some(k) = kind {
        (
            "SELECT id, kind, body, author, created_at FROM notes WHERE kind = ?1 ORDER BY created_at DESC".into(),
            vec![Box::new(k.to_string())],
        )
    } else {
        (
            "SELECT id, kind, body, author, created_at FROM notes ORDER BY created_at DESC".into(),
            vec![],
        )
    };

    let mut stmt = conn.prepare(&query)?;
    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    let notes: Vec<serde_json::Value> = stmt.query_map(param_refs.as_slice(), |row| {
        Ok(json!({
            "id": row.get::<_, String>(0)?,
            "kind": row.get::<_, String>(1)?,
            "body": row.get::<_, String>(2)?,
            "author": row.get::<_, String>(3)?,
            "created_at": row.get::<_, String>(4)?,
        }))
    })?.filter_map(|r| r.ok()).collect();

    if is_json {
        println!("{}", serde_json::to_string_pretty(&json!({ "ok": true, "notes": notes }))?);
    } else {
        if notes.is_empty() {
            println!("No notes.");
        } else {
            for n in &notes {
                println!("  [{}] {}: {}",
                    n["kind"].as_str().unwrap_or(""),
                    n["id"].as_str().unwrap_or(""),
                    n["body"].as_str().unwrap_or(""),
                );
            }
        }
    }
    Ok(())
}

fn remove(conn: &Connection, repo_id: &str, actor: &str, id: &str, is_json: bool) -> Result<(), LatchError> {
    let rows = conn.execute("DELETE FROM notes WHERE id = ?1", [id])?;

    if rows == 0 {
        return Err(LatchError::NotFound(format!("Note {id} not found")));
    }

    db::append_event(conn, repo_id, actor, "note.removed", "note", id, None, json!({}))?;

    if is_json {
        println!("{}", serde_json::to_string_pretty(&json!({ "ok": true, "removed": id }))?);
    } else {
        println!("Note {id} removed.");
    }
    Ok(())
}
