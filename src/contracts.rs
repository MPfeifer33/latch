use chrono::Utc;
use rusqlite::Connection;
use serde_json::json;
use std::path::Path;
use ulid::Ulid;

use crate::cli::{Cli, ContractCommand};
use crate::{db, output, LatchError};

pub fn handle(cmd: &ContractCommand, cli: &Cli) -> Result<(), LatchError> {
    let repo = cli.resolve_repo()?;
    let conn = db::open_workspace(&repo)?;
    let actor = cli.resolve_actor();
    let repo_id = db::compute_repo_id(&repo);

    match cmd {
        ContractCommand::Set {
            name,
            version,
            format,
            body,
            body_file,
            owner,
            consumer,
        } => set(
            &conn,
            &repo_id,
            &actor,
            name,
            version,
            format.as_deref(),
            body.as_deref(),
            body_file.as_deref(),
            owner.as_deref(),
            consumer,
            cli.is_json(),
        ),
        ContractCommand::List => list(&conn, cli.is_json()),
        ContractCommand::Get { name, version } => get(&conn, name, version.as_deref(), cli.is_json()),
    }
}

fn read_body(body: Option<&str>, body_file: Option<&Path>) -> Result<String, LatchError> {
    if let Some(path) = body_file {
        return std::fs::read_to_string(path).map_err(LatchError::Io);
    }
    Ok(body.unwrap_or("").to_string())
}

fn parse_json_array(raw: String) -> serde_json::Value {
    serde_json::from_str::<serde_json::Value>(&raw).unwrap_or_else(|_| json!([]))
}

fn body_value(format: &str, body: &str) -> Result<serde_json::Value, LatchError> {
    if format == "json" {
        if body.trim().is_empty() {
            return Ok(json!(null));
        }
        return serde_json::from_str(body).map_err(LatchError::Json);
    }
    Ok(json!(body))
}

fn set(
    conn: &Connection,
    repo_id: &str,
    actor: &str,
    name: &str,
    version: &str,
    format: Option<&str>,
    body: Option<&str>,
    body_file: Option<&Path>,
    owner: Option<&str>,
    consumers: &[String],
    is_json: bool,
) -> Result<(), LatchError> {
    let format = format.unwrap_or("json");
    if !["json", "markdown", "text"].contains(&format) {
        return Err(LatchError::Validation(format!(
            "Invalid contract format: {format}. Use json, markdown, or text"
        )));
    }

    let exists: i64 = conn.query_row(
        "SELECT COUNT(*) FROM contracts WHERE name = ?1 AND version = ?2",
        rusqlite::params![name, version],
        |row| row.get(0),
    )?;
    if exists > 0 {
        return Err(LatchError::Validation(format!(
            "Contract {name} {version} already exists; create a new version instead"
        )));
    }

    let body = read_body(body, body_file)?;
    if format == "json" && !body.trim().is_empty() {
        serde_json::from_str::<serde_json::Value>(&body)?;
    }

    let id = Ulid::new().to_string();
    let now = Utc::now().to_rfc3339();
    let owner = owner.unwrap_or(actor);
    let consumers_json = serde_json::to_string(consumers)?;

    conn.execute(
        "INSERT INTO contracts (id, name, version, format, body, owner, consumers, status, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'active', ?8)",
        rusqlite::params![id, name, version, format, body, owner, consumers_json, now],
    )?;

    db::append_event(
        conn,
        repo_id,
        actor,
        "contract.set",
        "contract",
        &id,
        None,
        json!({
            "name": name,
            "version": version,
            "format": format,
            "owner": owner,
            "consumers": consumers,
        }),
    )?;

    if is_json {
        output::print_json_value(json!({
            "ok": true,
            "contract": {
                "id": id,
                "name": name,
                "version": version,
                "format": format,
                "owner": owner,
                "consumers": consumers,
                "status": "active",
                "created_at": now,
            }
        }))?;
    } else {
        println!("Contract recorded: {name} {version} ({id})");
    }

    Ok(())
}

fn list(conn: &Connection, is_json: bool) -> Result<(), LatchError> {
    let mut stmt = conn.prepare(
        "SELECT id, name, version, format, owner, consumers, status, created_at
         FROM contracts
         WHERE status = 'active'
         ORDER BY name ASC, created_at DESC",
    )?;

    let contracts: Vec<serde_json::Value> = stmt
        .query_map([], |row| {
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
        })?
        .filter_map(|row| row.ok())
        .collect();

    if is_json {
        output::print_json_value(json!({ "ok": true, "contracts": contracts }))?;
    } else if contracts.is_empty() {
        println!("No contracts.");
    } else {
        for contract in &contracts {
            println!(
                "  {} {} ({})",
                contract["name"].as_str().unwrap_or(""),
                contract["version"].as_str().unwrap_or(""),
                contract["format"].as_str().unwrap_or(""),
            );
        }
    }

    Ok(())
}

fn get(conn: &Connection, name: &str, version: Option<&str>, is_json: bool) -> Result<(), LatchError> {
    let contract = if let Some(version) = version {
        load_contract(
            conn,
            "SELECT id, name, version, format, body, owner, consumers, status, created_at
             FROM contracts
             WHERE name = ?1 AND version = ?2",
            rusqlite::params![name, version],
            &format!("Contract {name} {version} not found"),
        )?
    } else {
        load_contract(
            conn,
            "SELECT id, name, version, format, body, owner, consumers, status, created_at
             FROM contracts
             WHERE name = ?1 AND status = 'active'
             ORDER BY created_at DESC
             LIMIT 1",
            rusqlite::params![name],
            &format!("Contract {name} not found"),
        )?
    };

    if is_json {
        output::print_json_value(json!({ "ok": true, "contract": contract }))?;
    } else {
        println!(
            "{} {} ({})",
            contract["name"].as_str().unwrap_or(""),
            contract["version"].as_str().unwrap_or(""),
            contract["format"].as_str().unwrap_or(""),
        );
        if let Some(owner) = contract["owner"].as_str() {
            if !owner.is_empty() {
                println!("owner: {owner}");
            }
        }
        println!();
        if contract["format"].as_str() == Some("json") {
            println!("{}", serde_json::to_string_pretty(&contract["body"])?);
        } else if let Some(body) = contract["body"].as_str() {
            println!("{body}");
        }
    }

    Ok(())
}

fn load_contract<P>(
    conn: &Connection,
    sql: &str,
    params: P,
    not_found: &str,
) -> Result<serde_json::Value, LatchError>
where
    P: rusqlite::Params,
{
    conn.query_row(sql, params, |row| {
        let format: String = row.get(3)?;
        let body: String = row.get(4)?;
        let body = body_value(&format, &body)
            .map_err(|err| rusqlite::Error::ToSqlConversionFailure(Box::new(err)))?;
        Ok(json!({
            "id": row.get::<_, String>(0)?,
            "name": row.get::<_, String>(1)?,
            "version": row.get::<_, String>(2)?,
            "format": format,
            "body": body,
            "owner": row.get::<_, String>(5)?,
            "consumers": parse_json_array(row.get::<_, String>(6)?),
            "status": row.get::<_, String>(7)?,
            "created_at": row.get::<_, String>(8)?,
        }))
    })
    .map_err(|_| LatchError::NotFound(not_found.to_string()))
}
