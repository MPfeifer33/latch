use rusqlite::Connection;
use serde_json::json;

use crate::cli::{Cli, EventsCommand};
use crate::db;
use crate::LatchError;

pub fn handle_cmd(cmd: &EventsCommand, cli: &Cli) -> Result<(), LatchError> {
    let repo = cli.resolve_repo()?;
    let conn = db::open_workspace(&repo)?;

    match cmd {
        EventsCommand::List { since, limit } => list_events(&conn, since.as_deref(), *limit, cli.is_json()),
        EventsCommand::Show { id } => show_event(&conn, id, cli.is_json()),
    }
}

fn list_events(conn: &Connection, since: Option<&str>, limit: usize, is_json: bool) -> Result<(), LatchError> {
    let mut query = String::from("SELECT id, created_at, actor, kind, entity_type, entity_id FROM events");
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

    if let Some(since) = since {
        query.push_str(" WHERE created_at >= ?1");
        params.push(Box::new(since.to_string()));
    }

    query.push_str(" ORDER BY created_at DESC LIMIT ?");
    let param_idx = params.len() + 1;
    query = query.replace("LIMIT ?", &format!("LIMIT ?{}", param_idx));
    params.push(Box::new(limit as i64));

    let mut stmt = conn.prepare(&query)?;
    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    let rows = stmt.query_map(param_refs.as_slice(), |row| {
        Ok(json!({
            "id": row.get::<_, String>(0)?,
            "created_at": row.get::<_, String>(1)?,
            "actor": row.get::<_, String>(2)?,
            "kind": row.get::<_, String>(3)?,
            "entity_type": row.get::<_, String>(4)?,
            "entity_id": row.get::<_, String>(5)?,
        }))
    })?;

    let events: Vec<serde_json::Value> = rows.filter_map(|r| r.ok()).collect();

    if is_json {
        println!("{}", serde_json::to_string_pretty(&json!({ "ok": true, "events": events }))?);
    } else {
        for e in &events {
            println!("[{}] {} {} {} -> {}",
                e["created_at"].as_str().unwrap_or(""),
                e["actor"].as_str().unwrap_or(""),
                e["kind"].as_str().unwrap_or(""),
                e["entity_type"].as_str().unwrap_or(""),
                e["entity_id"].as_str().unwrap_or(""),
            );
        }
    }
    Ok(())
}

fn show_event(conn: &Connection, id: &str, is_json: bool) -> Result<(), LatchError> {
    let event = conn.query_row(
        "SELECT id, repo_id, created_at, actor, kind, entity_type, entity_id, correlation_id, payload FROM events WHERE id = ?1",
        [id],
        |row| {
            Ok(json!({
                "id": row.get::<_, String>(0)?,
                "repo_id": row.get::<_, String>(1)?,
                "created_at": row.get::<_, String>(2)?,
                "actor": row.get::<_, String>(3)?,
                "kind": row.get::<_, String>(4)?,
                "entity_type": row.get::<_, String>(5)?,
                "entity_id": row.get::<_, String>(6)?,
                "correlation_id": row.get::<_, Option<String>>(7)?,
                "payload": row.get::<_, String>(8)?,
            }))
        },
    ).map_err(|_| LatchError::NotFound(format!("Event {id} not found")))?;

    if is_json {
        println!("{}", serde_json::to_string_pretty(&json!({ "ok": true, "event": event }))?);
    } else {
        println!("{}", serde_json::to_string_pretty(&event)?);
    }
    Ok(())
}
