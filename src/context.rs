use chrono::Utc;
use rusqlite::{types::ToSql, Connection};
use serde_json::{json, Value};
use std::path::Path;

use crate::{output, LatchError};

pub fn show_status(
    conn: &Connection,
    actor: Option<&str>,
    is_json: bool,
) -> Result<(), LatchError> {
    let claims = active_claims(conn, actor)?;
    let tasks = active_tasks(conn, actor)?;
    let decisions = recent_decisions(conn, 8)?;
    let contracts = active_contracts(conn, 8)?;
    let hazards = hazards(conn, 8)?;

    if is_json {
        output::print_json_value(json!({
            "ok": true,
            "status": {
                "actor": actor,
                "active_claims": claims,
                "tasks": tasks,
                "recent_decisions": decisions,
                "contracts": contracts,
                "hazards": hazards,
            }
        }))?;
    } else {
        print_status_text(actor, &claims, &tasks, &decisions, &contracts, &hazards);
    }

    Ok(())
}

pub fn show_context(
    conn: &Connection,
    repo: &Path,
    actor: Option<&str>,
    is_json: bool,
) -> Result<(), LatchError> {
    let claims = active_claims(conn, actor)?;
    let tasks = active_tasks(conn, actor)?;
    let decisions = recent_decisions(conn, 5)?;
    let contracts = active_contracts(conn, 5)?;
    let hazards = hazards(conn, 5)?;
    let repo = repo.display().to_string();

    if is_json {
        output::print_json_value(json!({
            "ok": true,
            "context": {
                "actor": actor,
                "repo": repo,
                "active_claims": claims,
                "assigned_tasks": tasks,
                "recent_decisions": decisions,
                "contracts": contracts,
                "hazards": hazards,
            }
        }))?;
    } else {
        print_context_text(
            actor, &repo, &claims, &tasks, &decisions, &contracts, &hazards,
        );
    }

    Ok(())
}

fn active_claims(conn: &Connection, actor: Option<&str>) -> Result<Vec<Value>, LatchError> {
    let now = Utc::now().to_rfc3339();
    let (sql, params): (&str, Vec<Box<dyn ToSql>>) = if let Some(actor) = actor {
        (
            "SELECT id, owner, path, scope, intent, acquired_at, expires_at
             FROM claims
             WHERE status = 'active' AND expires_at > ?1 AND owner = ?2
             ORDER BY expires_at ASC",
            vec![Box::new(now), Box::new(actor.to_string())],
        )
    } else {
        (
            "SELECT id, owner, path, scope, intent, acquired_at, expires_at
             FROM claims
             WHERE status = 'active' AND expires_at > ?1
             ORDER BY expires_at ASC",
            vec![Box::new(now)],
        )
    };

    query_values(conn, sql, params, |row| {
        Ok(json!({
            "id": row.get::<_, String>(0)?,
            "owner": row.get::<_, String>(1)?,
            "path": row.get::<_, String>(2)?,
            "scope": row.get::<_, String>(3)?,
            "intent": row.get::<_, String>(4)?,
            "acquired_at": row.get::<_, String>(5)?,
            "expires_at": row.get::<_, String>(6)?,
        }))
    })
}

fn active_tasks(conn: &Connection, actor: Option<&str>) -> Result<Vec<Value>, LatchError> {
    let (sql, params): (&str, Vec<Box<dyn ToSql>>) = if let Some(actor) = actor {
        (
            "SELECT id, title, assigned_to, created_by, status, priority, created_at, updated_at
             FROM tasks
             WHERE status IN ('open', 'taken') AND assigned_to = ?1
             ORDER BY created_at DESC",
            vec![Box::new(actor.to_string())],
        )
    } else {
        (
            "SELECT id, title, assigned_to, created_by, status, priority, created_at, updated_at
             FROM tasks
             WHERE status IN ('open', 'taken')
             ORDER BY created_at DESC",
            vec![],
        )
    };

    query_values(conn, sql, params, |row| {
        Ok(json!({
            "id": row.get::<_, String>(0)?,
            "title": row.get::<_, String>(1)?,
            "assigned_to": row.get::<_, String>(2)?,
            "created_by": row.get::<_, String>(3)?,
            "status": row.get::<_, String>(4)?,
            "priority": row.get::<_, String>(5)?,
            "created_at": row.get::<_, String>(6)?,
            "updated_at": row.get::<_, String>(7)?,
        }))
    })
}

fn recent_decisions(conn: &Connection, limit: i64) -> Result<Vec<Value>, LatchError> {
    query_values(
        conn,
        "SELECT id, title, participants, tags, status, created_at, superseded_by
         FROM decisions
         ORDER BY created_at DESC
         LIMIT ?1",
        vec![Box::new(limit)],
        |row| {
            Ok(json!({
                "id": row.get::<_, String>(0)?,
                "title": row.get::<_, String>(1)?,
                "participants": parse_json_array(row.get::<_, String>(2)?),
                "tags": parse_json_array(row.get::<_, String>(3)?),
                "status": row.get::<_, String>(4)?,
                "created_at": row.get::<_, String>(5)?,
                "superseded_by": row.get::<_, Option<String>>(6)?,
            }))
        },
    )
}

fn active_contracts(conn: &Connection, limit: i64) -> Result<Vec<Value>, LatchError> {
    query_values(
        conn,
        "SELECT id, name, version, format, owner, consumers, status, created_at
         FROM contracts
         WHERE status = 'active'
         ORDER BY created_at DESC
         LIMIT ?1",
        vec![Box::new(limit)],
        |row| {
            Ok(json!({
                "id": row.get::<_, String>(0)?,
                "name": row.get::<_, String>(1)?,
                "version": row.get::<_, String>(2)?,
                "format": row.get::<_, String>(3)?,
                "owner": row.get::<_, String>(4)?,
                "consumers": parse_json_array(row.get::<_, String>(5)?),
                "status": row.get::<_, String>(6)?,
                "created_at": row.get::<_, String>(7)?,
            }))
        },
    )
}

fn hazards(conn: &Connection, limit: i64) -> Result<Vec<Value>, LatchError> {
    query_values(
        conn,
        "SELECT id, body, author, created_at
         FROM notes
         WHERE kind = 'hazard'
         ORDER BY created_at DESC
         LIMIT ?1",
        vec![Box::new(limit)],
        |row| {
            Ok(json!({
                "id": row.get::<_, String>(0)?,
                "kind": "hazard",
                "body": row.get::<_, String>(1)?,
                "author": row.get::<_, String>(2)?,
                "created_at": row.get::<_, String>(3)?,
            }))
        },
    )
}

fn query_values<F>(
    conn: &Connection,
    sql: &str,
    params: Vec<Box<dyn ToSql>>,
    mapper: F,
) -> Result<Vec<Value>, LatchError>
where
    F: FnMut(&rusqlite::Row<'_>) -> rusqlite::Result<Value>,
{
    let mut stmt = conn.prepare(sql)?;
    let param_refs: Vec<&dyn ToSql> = params.iter().map(|p| p.as_ref()).collect();
    let rows = stmt.query_map(param_refs.as_slice(), mapper)?;
    let mut values = Vec::new();
    for row in rows {
        values.push(row?);
    }
    Ok(values)
}

fn parse_json_array(raw: String) -> Value {
    serde_json::from_str::<Value>(&raw).unwrap_or_else(|_| json!([]))
}

fn print_status_text(
    actor: Option<&str>,
    claims: &[Value],
    tasks: &[Value],
    decisions: &[Value],
    contracts: &[Value],
    hazards: &[Value],
) {
    if let Some(actor) = actor {
        println!("Latch status for {actor}");
    } else {
        println!("Latch status");
    }

    print_group("Active claims", claim_lines(claims));
    print_group("Tasks", task_lines(tasks));
    print_group("Recent decisions", decision_lines(decisions));
    print_group("Contracts", contract_lines(contracts));
    print_group("Hazards", hazard_lines(hazards));
}

fn print_context_text(
    actor: Option<&str>,
    repo: &str,
    claims: &[Value],
    tasks: &[Value],
    decisions: &[Value],
    contracts: &[Value],
    hazards: &[Value],
) {
    match actor {
        Some(actor) => println!("Latch context for {actor} in {repo}"),
        None => println!("Latch context for all actors in {repo}"),
    }
    println!("Active claims: {}", inline_or_none(claim_lines(claims)));
    println!("Assigned tasks: {}", inline_or_none(task_lines(tasks)));
    println!(
        "Recent decisions: {}",
        inline_or_none(decision_lines(decisions))
    );
    println!("Contracts: {}", inline_or_none(contract_lines(contracts)));
    println!("Hazards: {}", inline_or_none(hazard_lines(hazards)));
}

fn print_group(label: &str, lines: Vec<String>) {
    println!("{label}:");
    if lines.is_empty() {
        println!("  none");
    } else {
        for line in lines {
            println!("  - {line}");
        }
    }
}

fn inline_or_none(lines: Vec<String>) -> String {
    if lines.is_empty() {
        "none".to_string()
    } else {
        lines.join("; ")
    }
}

fn claim_lines(claims: &[Value]) -> Vec<String> {
    claims
        .iter()
        .map(|claim| {
            let intent = field(claim, "intent");
            let intent = if intent.is_empty() {
                String::new()
            } else {
                format!(" ({intent})")
            };
            format!(
                "{} {} until {}{}",
                field(claim, "owner"),
                field(claim, "path"),
                field(claim, "expires_at"),
                intent
            )
        })
        .collect()
}

fn task_lines(tasks: &[Value]) -> Vec<String> {
    tasks
        .iter()
        .map(|task| {
            format!(
                "[{}/{}] {} ({} -> {})",
                field(task, "status"),
                field(task, "priority"),
                field(task, "title"),
                field(task, "created_by"),
                field(task, "assigned_to")
            )
        })
        .collect()
}

fn decision_lines(decisions: &[Value]) -> Vec<String> {
    decisions
        .iter()
        .map(|decision| {
            format!(
                "[{}] {} ({})",
                field(decision, "status"),
                field(decision, "title"),
                field(decision, "id")
            )
        })
        .collect()
}

fn contract_lines(contracts: &[Value]) -> Vec<String> {
    contracts
        .iter()
        .map(|contract| {
            format!(
                "{} {} owned by {} ({})",
                field(contract, "name"),
                field(contract, "version"),
                field(contract, "owner"),
                field(contract, "format")
            )
        })
        .collect()
}

fn hazard_lines(hazards: &[Value]) -> Vec<String> {
    hazards
        .iter()
        .map(|hazard| format!("{} ({})", field(hazard, "body"), field(hazard, "author")))
        .collect()
}

fn field<'a>(value: &'a Value, key: &str) -> &'a str {
    value[key].as_str().unwrap_or("")
}
