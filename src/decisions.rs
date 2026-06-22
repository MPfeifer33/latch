use chrono::Utc;
use rusqlite::Connection;
use serde_json::json;
use std::path::Path;
use ulid::Ulid;

use crate::cli::{Cli, DecisionCommand};
use crate::{db, output, LatchError};

pub fn handle(cmd: &DecisionCommand, cli: &Cli) -> Result<(), LatchError> {
    let repo = cli.resolve_repo()?;
    let conn = db::open_workspace(&repo)?;
    let actor = cli.resolve_actor();
    let repo_id = db::compute_repo_id(&repo);

    match cmd {
        DecisionCommand::Add {
            title,
            body,
            body_file,
            tag,
            participant,
        } => add(
            &conn,
            &repo_id,
            &actor,
            title,
            body.as_deref(),
            body_file.as_deref(),
            tag,
            participant,
            cli.is_json(),
        ),
        DecisionCommand::List => list(&conn, cli.is_json()),
        DecisionCommand::Show { id } => show(&conn, id, cli.is_json()),
        DecisionCommand::Supersede {
            id,
            title,
            body,
            body_file,
        } => supersede(
            &conn,
            &repo_id,
            &actor,
            id,
            title,
            body.as_deref(),
            body_file.as_deref(),
            cli.is_json(),
        ),
    }
}

fn read_body(body: Option<&str>, body_file: Option<&Path>) -> Result<String, LatchError> {
    if let Some(path) = body_file {
        return std::fs::read_to_string(path).map_err(LatchError::Io);
    }
    Ok(body.unwrap_or("").to_string())
}

fn json_array(values: &[String]) -> Result<String, LatchError> {
    serde_json::to_string(values).map_err(LatchError::Json)
}

fn parse_json_array(raw: String) -> serde_json::Value {
    serde_json::from_str::<serde_json::Value>(&raw).unwrap_or_else(|_| json!([]))
}

fn add(
    conn: &Connection,
    repo_id: &str,
    actor: &str,
    title: &str,
    body: Option<&str>,
    body_file: Option<&Path>,
    tags: &[String],
    participants: &[String],
    is_json: bool,
) -> Result<(), LatchError> {
    let id = Ulid::new().to_string();
    let now = Utc::now().to_rfc3339();
    let body = read_body(body, body_file)?;
    let tags_json = json_array(tags)?;
    let participants_json = json_array(participants)?;

    conn.execute(
        "INSERT INTO decisions (id, title, body, participants, tags, status, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, 'active', ?6)",
        rusqlite::params![id, title, body, participants_json, tags_json, now],
    )?;

    db::append_event(
        conn,
        repo_id,
        actor,
        "decision.added",
        "decision",
        &id,
        None,
        json!({
            "title": title,
            "participants": participants,
            "tags": tags,
        }),
    )?;

    if is_json {
        output::print_json_value(json!({
            "ok": true,
            "decision": {
                "id": id,
                "title": title,
                "status": "active",
                "created_at": now,
                "participants": participants,
                "tags": tags,
            }
        }))?;
    } else {
        println!("Decision recorded: {id} {title}");
    }

    Ok(())
}

fn list(conn: &Connection, is_json: bool) -> Result<(), LatchError> {
    let mut stmt = conn.prepare(
        "SELECT id, title, participants, tags, status, created_at, superseded_by
         FROM decisions
         ORDER BY created_at DESC",
    )?;

    let decisions: Vec<serde_json::Value> = stmt
        .query_map([], |row| {
            Ok(json!({
                "id": row.get::<_, String>(0)?,
                "title": row.get::<_, String>(1)?,
                "participants": parse_json_array(row.get::<_, String>(2)?),
                "tags": parse_json_array(row.get::<_, String>(3)?),
                "status": row.get::<_, String>(4)?,
                "created_at": row.get::<_, String>(5)?,
                "superseded_by": row.get::<_, Option<String>>(6)?,
            }))
        })?
        .filter_map(|row| row.ok())
        .collect();

    if is_json {
        output::print_json_value(json!({ "ok": true, "decisions": decisions }))?;
    } else if decisions.is_empty() {
        println!("No decisions.");
    } else {
        for decision in &decisions {
            println!(
                "  [{}] {} {}",
                decision["status"].as_str().unwrap_or(""),
                decision["id"].as_str().unwrap_or(""),
                decision["title"].as_str().unwrap_or(""),
            );
        }
    }

    Ok(())
}

fn show(conn: &Connection, id: &str, is_json: bool) -> Result<(), LatchError> {
    let decision = load_decision(conn, id)?;

    if is_json {
        output::print_json_value(json!({ "ok": true, "decision": decision }))?;
    } else {
        println!("{} {}", decision["id"].as_str().unwrap_or(""), decision["title"].as_str().unwrap_or(""));
        println!("status: {}", decision["status"].as_str().unwrap_or(""));
        println!("created: {}", decision["created_at"].as_str().unwrap_or(""));
        if let Some(superseded_by) = decision["superseded_by"].as_str() {
            println!("superseded by: {superseded_by}");
        }
        if let Some(body) = decision["body"].as_str() {
            if !body.is_empty() {
                println!();
                println!("{body}");
            }
        }
    }

    Ok(())
}

fn supersede(
    conn: &Connection,
    repo_id: &str,
    actor: &str,
    old_id: &str,
    title: &str,
    body: Option<&str>,
    body_file: Option<&Path>,
    is_json: bool,
) -> Result<(), LatchError> {
    let old = load_decision(conn, old_id)?;
    if old["status"].as_str() != Some("active") {
        return Err(LatchError::Validation(format!(
            "Decision {old_id} is not active and cannot be superseded"
        )));
    }

    let new_id = Ulid::new().to_string();
    let now = Utc::now().to_rfc3339();
    let body = read_body(body, body_file)?;
    let participants = serde_json::to_string(&old["participants"])?;
    let tags = serde_json::to_string(&old["tags"])?;

    conn.execute(
        "INSERT INTO decisions (id, title, body, participants, tags, status, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, 'active', ?6)",
        rusqlite::params![new_id, title, body, participants, tags, now],
    )?;
    conn.execute(
        "UPDATE decisions SET status = 'superseded', superseded_by = ?1 WHERE id = ?2",
        rusqlite::params![new_id, old_id],
    )?;

    db::append_event(
        conn,
        repo_id,
        actor,
        "decision.superseded",
        "decision",
        old_id,
        Some(&new_id),
        json!({
            "superseded_by": new_id,
            "title": title,
        }),
    )?;

    if is_json {
        output::print_json_value(json!({
            "ok": true,
            "superseded": old_id,
            "decision": {
                "id": new_id,
                "title": title,
                "status": "active",
                "created_at": now,
            }
        }))?;
    } else {
        println!("Decision {old_id} superseded by {new_id}: {title}");
    }

    Ok(())
}

fn load_decision(conn: &Connection, id: &str) -> Result<serde_json::Value, LatchError> {
    conn.query_row(
        "SELECT id, title, body, participants, tags, status, created_at, superseded_by
         FROM decisions
         WHERE id = ?1",
        [id],
        |row| {
            Ok(json!({
                "id": row.get::<_, String>(0)?,
                "title": row.get::<_, String>(1)?,
                "body": row.get::<_, String>(2)?,
                "participants": parse_json_array(row.get::<_, String>(3)?),
                "tags": parse_json_array(row.get::<_, String>(4)?),
                "status": row.get::<_, String>(5)?,
                "created_at": row.get::<_, String>(6)?,
                "superseded_by": row.get::<_, Option<String>>(7)?,
            }))
        },
    )
    .map_err(|_| LatchError::NotFound(format!("Decision {id} not found")))
}
