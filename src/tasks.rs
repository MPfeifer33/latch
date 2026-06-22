use chrono::Utc;
use rusqlite::Connection;
use serde_json::json;
use ulid::Ulid;

use crate::cli::{Cli, TaskCommand};
use crate::db;
use crate::LatchError;

pub fn handle(cmd: &TaskCommand, cli: &Cli) -> Result<(), LatchError> {
    let repo = cli.resolve_repo()?;
    let conn = db::open_workspace(&repo)?;
    let actor = cli.resolve_actor();
    let repo_id = db::compute_repo_id(&repo);

    match cmd {
        TaskCommand::Add { to, title, body, priority } => {
            add(&conn, &repo_id, &actor, to, title, body.as_deref(), priority.as_deref(), cli.is_json())
        }
        TaskCommand::List { r#for: assignee } => list(&conn, assignee.as_deref(), cli.is_json()),
        TaskCommand::Take { id } => transition(&conn, &repo_id, &actor, id, "open", "taken", "task.taken", cli.is_json()),
        TaskCommand::Done { id } => transition(&conn, &repo_id, &actor, id, "taken", "done", "task.done", cli.is_json()),
        TaskCommand::Cancel { id } => cancel(&conn, &repo_id, &actor, id, cli.is_json()),
    }
}

fn add(
    conn: &Connection,
    repo_id: &str,
    actor: &str,
    to: &str,
    title: &str,
    body: Option<&str>,
    priority: Option<&str>,
    is_json: bool,
) -> Result<(), LatchError> {
    let id = Ulid::new().to_string();
    let now = Utc::now().to_rfc3339();
    let priority = priority.unwrap_or("normal");

    conn.execute(
        "INSERT INTO tasks (id, title, body, assigned_to, created_by, status, priority, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, 'open', ?6, ?7, ?7)",
        rusqlite::params![id, title, body.unwrap_or(""), to, actor, priority, now],
    )?;

    db::append_event(conn, repo_id, actor, "task.added", "task", &id, None, json!({
        "title": title,
        "assigned_to": to,
        "priority": priority,
    }))?;

    if is_json {
        println!("{}", serde_json::to_string_pretty(&json!({
            "ok": true,
            "task": { "id": id, "title": title, "assigned_to": to, "status": "open" }
        }))?);
    } else {
        println!("Task created: {id} -> {to}: {title}");
    }
    Ok(())
}

fn list(conn: &Connection, assignee: Option<&str>, is_json: bool) -> Result<(), LatchError> {
    let (query, params): (String, Vec<Box<dyn rusqlite::types::ToSql>>) = if let Some(a) = assignee {
        (
            "SELECT id, title, assigned_to, created_by, status, priority, created_at FROM tasks WHERE assigned_to = ?1 AND status IN ('open', 'taken') ORDER BY created_at DESC".into(),
            vec![Box::new(a.to_string())],
        )
    } else {
        (
            "SELECT id, title, assigned_to, created_by, status, priority, created_at FROM tasks WHERE status IN ('open', 'taken') ORDER BY created_at DESC".into(),
            vec![],
        )
    };

    let mut stmt = conn.prepare(&query)?;
    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    let tasks: Vec<serde_json::Value> = stmt.query_map(param_refs.as_slice(), |row| {
        Ok(json!({
            "id": row.get::<_, String>(0)?,
            "title": row.get::<_, String>(1)?,
            "assigned_to": row.get::<_, String>(2)?,
            "created_by": row.get::<_, String>(3)?,
            "status": row.get::<_, String>(4)?,
            "priority": row.get::<_, String>(5)?,
            "created_at": row.get::<_, String>(6)?,
        }))
    })?.filter_map(|r| r.ok()).collect();

    if is_json {
        println!("{}", serde_json::to_string_pretty(&json!({ "ok": true, "tasks": tasks }))?);
    } else {
        if tasks.is_empty() {
            println!("No open tasks.");
        } else {
            for t in &tasks {
                println!("  [{}] {} -> {}: {}",
                    t["status"].as_str().unwrap_or(""),
                    t["created_by"].as_str().unwrap_or(""),
                    t["assigned_to"].as_str().unwrap_or(""),
                    t["title"].as_str().unwrap_or(""),
                );
            }
        }
    }
    Ok(())
}

fn transition(
    conn: &Connection,
    repo_id: &str,
    actor: &str,
    id: &str,
    from_status: &str,
    to_status: &str,
    event_kind: &str,
    is_json: bool,
) -> Result<(), LatchError> {
    let now = Utc::now().to_rfc3339();
    let rows = conn.execute(
        "UPDATE tasks SET status = ?1, updated_at = ?2 WHERE id = ?3 AND status = ?4",
        rusqlite::params![to_status, now, id, from_status],
    )?;

    if rows == 0 {
        return Err(LatchError::NotFound(format!("Task {id} not found or not in '{from_status}' status")));
    }

    db::append_event(conn, repo_id, actor, event_kind, "task", id, None, json!({
        "from": from_status,
        "to": to_status,
    }))?;

    if is_json {
        println!("{}", serde_json::to_string_pretty(&json!({ "ok": true, "task": id, "status": to_status }))?);
    } else {
        println!("Task {id} -> {to_status}");
    }
    Ok(())
}

fn cancel(conn: &Connection, repo_id: &str, actor: &str, id: &str, is_json: bool) -> Result<(), LatchError> {
    let now = Utc::now().to_rfc3339();
    let rows = conn.execute(
        "UPDATE tasks SET status = 'canceled', updated_at = ?1 WHERE id = ?2 AND status IN ('open', 'taken')",
        rusqlite::params![now, id],
    )?;

    if rows == 0 {
        return Err(LatchError::NotFound(format!("Task {id} not found or already completed")));
    }

    db::append_event(conn, repo_id, actor, "task.canceled", "task", id, None, json!({}))?;

    if is_json {
        println!("{}", serde_json::to_string_pretty(&json!({ "ok": true, "task": id, "status": "canceled" }))?);
    } else {
        println!("Task {id} canceled.");
    }
    Ok(())
}
