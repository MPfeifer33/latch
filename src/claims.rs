use chrono::{Utc, Duration};
use rusqlite::Connection;
use serde_json::json;
use ulid::Ulid;

use crate::cli::{Cli, ClaimCommand};
use crate::db;
use crate::LatchError;

pub fn handle(cmd: &ClaimCommand, cli: &Cli) -> Result<(), LatchError> {
    let repo = cli.resolve_repo()?;
    let conn = db::open_workspace(&repo)?;
    let actor = cli.resolve_actor();
    let repo_id = db::compute_repo_id(&repo);

    match cmd {
        ClaimCommand::Acquire { path, intent, ttl } => {
            acquire(&conn, &repo_id, &actor, path, intent.as_deref(), ttl, cli.is_json())
        }
        ClaimCommand::List => list(&conn, cli.is_json()),
        ClaimCommand::Renew { id, ttl } => renew(&conn, &repo_id, &actor, id, ttl, cli.is_json()),
        ClaimCommand::Release { id } => release(&conn, &repo_id, &actor, id, cli.is_json()),
    }
}

fn parse_ttl(ttl: &str) -> Result<Duration, LatchError> {
    let ttl = ttl.trim();
    if let Some(hours) = ttl.strip_suffix('h') {
        let h: i64 = hours.parse().map_err(|_| LatchError::Validation(format!("Invalid TTL: {ttl}")))?;
        Ok(Duration::hours(h))
    } else if let Some(mins) = ttl.strip_suffix('m') {
        let m: i64 = mins.parse().map_err(|_| LatchError::Validation(format!("Invalid TTL: {ttl}")))?;
        Ok(Duration::minutes(m))
    } else {
        Err(LatchError::Validation(format!("Invalid TTL format: {ttl}. Use e.g. '2h' or '30m'")))
    }
}

fn normalize_path(path: &str) -> Result<String, LatchError> {
    let path = path.replace('\\', "/");
    if path.contains("..") {
        return Err(LatchError::Validation("Path must not contain traversal (..)".into()));
    }
    let path = path.trim_start_matches('/').to_string();
    Ok(path)
}

fn check_conflicts(conn: &Connection, path: &str) -> Result<Vec<serde_json::Value>, LatchError> {
    let now = Utc::now().to_rfc3339();
    let mut stmt = conn.prepare(
        "SELECT id, owner, path, expires_at FROM claims WHERE status = 'active' AND expires_at > ?1"
    )?;

    let conflicts: Vec<serde_json::Value> = stmt.query_map([&now], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
        ))
    })?
    .filter_map(|r| r.ok())
    .filter(|(_, _, existing_path, _)| paths_conflict(existing_path, path))
    .map(|(id, owner, p, expires)| json!({
        "id": id,
        "owner": owner,
        "path": p,
        "expires_at": expires,
    }))
    .collect();

    Ok(conflicts)
}

fn paths_conflict(a: &str, b: &str) -> bool {
    // Exact match
    if a == b {
        return true;
    }
    // Directory containment: a/ contains b, or b/ contains a
    let a_dir = if a.ends_with('/') { a.to_string() } else { format!("{a}/") };
    let b_dir = if b.ends_with('/') { b.to_string() } else { format!("{b}/") };

    b.starts_with(&a_dir) || a.starts_with(&b_dir)
}

fn acquire(
    conn: &Connection,
    repo_id: &str,
    actor: &str,
    path: &str,
    intent: Option<&str>,
    ttl: &str,
    is_json: bool,
) -> Result<(), LatchError> {
    let path = normalize_path(path)?;
    let duration = parse_ttl(ttl)?;

    let conflicts = check_conflicts(conn, &path)?;
    if !conflicts.is_empty() {
        if is_json {
            let err = json!({
                "ok": false,
                "error": {
                    "code": "claim_conflict",
                    "message": "Path is already claimed",
                    "conflicts": conflicts,
                }
            });
            eprintln!("{}", serde_json::to_string_pretty(&err)?);
        } else {
            eprintln!("error: claim conflict on {path}");
        }
        return Err(LatchError::ClaimConflict(format!("Path {path} is already claimed")));
    }

    let now = Utc::now();
    let expires = now + duration;
    let id = Ulid::new().to_string();
    let scope = if path.ends_with('/') { "dir" } else { "file" };

    conn.execute(
        "INSERT INTO claims (id, owner, path, scope, intent, status, acquired_at, heartbeat_at, expires_at)
         VALUES (?1, ?2, ?3, ?4, ?5, 'active', ?6, ?6, ?7)",
        rusqlite::params![id, actor, path, scope, intent.unwrap_or(""), now.to_rfc3339(), expires.to_rfc3339()],
    )?;

    db::append_event(conn, repo_id, actor, "claim.acquired", "claim", &id, None, json!({
        "path": path,
        "scope": scope,
        "intent": intent.unwrap_or(""),
        "ttl": ttl,
    }))?;

    if is_json {
        println!("{}", serde_json::to_string_pretty(&json!({
            "ok": true,
            "claim": {
                "id": id,
                "owner": actor,
                "path": path,
                "scope": scope,
                "expires_at": expires.to_rfc3339(),
            }
        }))?);
    } else {
        println!("Claim acquired: {id} on {path} (expires {})", expires.to_rfc3339());
    }

    Ok(())
}

fn list(conn: &Connection, is_json: bool) -> Result<(), LatchError> {
    let now = Utc::now().to_rfc3339();
    let mut stmt = conn.prepare(
        "SELECT id, owner, path, scope, intent, status, acquired_at, expires_at FROM claims WHERE status = 'active' AND expires_at > ?1 ORDER BY acquired_at DESC"
    )?;

    let claims: Vec<serde_json::Value> = stmt.query_map([&now], |row| {
        Ok(json!({
            "id": row.get::<_, String>(0)?,
            "owner": row.get::<_, String>(1)?,
            "path": row.get::<_, String>(2)?,
            "scope": row.get::<_, String>(3)?,
            "intent": row.get::<_, String>(4)?,
            "status": row.get::<_, String>(5)?,
            "acquired_at": row.get::<_, String>(6)?,
            "expires_at": row.get::<_, String>(7)?,
        }))
    })?.filter_map(|r| r.ok()).collect();

    if is_json {
        println!("{}", serde_json::to_string_pretty(&json!({ "ok": true, "claims": claims }))?);
    } else {
        if claims.is_empty() {
            println!("No active claims.");
        } else {
            for c in &claims {
                println!("  {} {} {} (until {})",
                    c["owner"].as_str().unwrap_or(""),
                    c["path"].as_str().unwrap_or(""),
                    c["intent"].as_str().unwrap_or(""),
                    c["expires_at"].as_str().unwrap_or(""),
                );
            }
        }
    }
    Ok(())
}

fn renew(conn: &Connection, repo_id: &str, actor: &str, id: &str, ttl: &str, is_json: bool) -> Result<(), LatchError> {
    let duration = parse_ttl(ttl)?;
    let now = Utc::now();
    let new_expires = now + duration;

    let rows = conn.execute(
        "UPDATE claims SET heartbeat_at = ?1, expires_at = ?2 WHERE id = ?3 AND status = 'active'",
        rusqlite::params![now.to_rfc3339(), new_expires.to_rfc3339(), id],
    )?;

    if rows == 0 {
        return Err(LatchError::NotFound(format!("Claim {id} not found or not active")));
    }

    db::append_event(conn, repo_id, actor, "claim.renewed", "claim", id, None, json!({
        "new_expires_at": new_expires.to_rfc3339(),
        "ttl": ttl,
    }))?;

    if is_json {
        println!("{}", serde_json::to_string_pretty(&json!({ "ok": true, "renewed": id, "expires_at": new_expires.to_rfc3339() }))?);
    } else {
        println!("Claim {id} renewed until {}", new_expires.to_rfc3339());
    }
    Ok(())
}

fn release(conn: &Connection, repo_id: &str, actor: &str, id: &str, is_json: bool) -> Result<(), LatchError> {
    let now = Utc::now().to_rfc3339();

    let rows = conn.execute(
        "UPDATE claims SET status = 'released', released_at = ?1 WHERE id = ?2 AND status = 'active'",
        rusqlite::params![now, id],
    )?;

    if rows == 0 {
        return Err(LatchError::NotFound(format!("Claim {id} not found or not active")));
    }

    db::append_event(conn, repo_id, actor, "claim.released", "claim", id, None, json!({}))?;

    if is_json {
        println!("{}", serde_json::to_string_pretty(&json!({ "ok": true, "released": id }))?);
    } else {
        println!("Claim {id} released.");
    }
    Ok(())
}
