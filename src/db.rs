use std::path::{Path, PathBuf};
use rusqlite::Connection;
use chrono::Utc;
use ulid::Ulid;

use crate::LatchError;

const WORKSPACE_DIR: &str = ".agent-workspace";
const DB_FILE: &str = "workspace.sqlite";

pub fn workspace_path(repo: &Path) -> PathBuf {
    repo.join(WORKSPACE_DIR).join(DB_FILE)
}

pub fn init_workspace(repo: &Path) -> Result<Connection, LatchError> {
    let ws_dir = repo.join(WORKSPACE_DIR);
    std::fs::create_dir_all(&ws_dir)?;

    // Write .gitignore if it doesn't exist
    let gitignore = ws_dir.join(".gitignore");
    if !gitignore.exists() {
        std::fs::write(&gitignore, "*\n")?;
    }

    let db_path = ws_dir.join(DB_FILE);
    let conn = Connection::open(&db_path)?;
    configure_connection(&conn)?;
    run_migrations(&conn)?;

    // Append workspace.initialized event
    let repo_id = compute_repo_id(repo);
    append_event(
        &conn,
        &repo_id,
        "system",
        "workspace.initialized",
        "workspace",
        "workspace",
        None,
        serde_json::json!({}),
    )?;

    Ok(conn)
}

pub fn open_workspace(repo: &Path) -> Result<Connection, LatchError> {
    let db_path = workspace_path(repo);
    if !db_path.exists() {
        return Err(LatchError::Storage(format!(
            "No workspace found at {}. Run `latch init` first.",
            db_path.display()
        )));
    }
    let conn = Connection::open(&db_path)?;
    configure_connection(&conn)?;
    Ok(conn)
}

fn configure_connection(conn: &Connection) -> Result<(), LatchError> {
    conn.execute_batch(
        "PRAGMA journal_mode = WAL;
         PRAGMA foreign_keys = ON;
         PRAGMA busy_timeout = 5000;"
    )?;
    Ok(())
}

fn run_migrations(conn: &Connection) -> Result<(), LatchError> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
            version INTEGER PRIMARY KEY,
            applied_at TEXT NOT NULL
        );"
    )?;

    let current_version: i64 = conn
        .query_row(
            "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
            [],
            |row| row.get(0),
        )?;

    if current_version < 1 {
        conn.execute_batch(include_str!("../migrations/001_initial.sql"))?;
        conn.execute(
            "INSERT INTO schema_migrations (version, applied_at) VALUES (1, ?1)",
            [Utc::now().to_rfc3339()],
        )?;
    }

    Ok(())
}

pub fn append_event(
    conn: &Connection,
    repo_id: &str,
    actor: &str,
    kind: &str,
    entity_type: &str,
    entity_id: &str,
    correlation_id: Option<&str>,
    payload: serde_json::Value,
) -> Result<String, LatchError> {
    let id = Ulid::new().to_string();
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO events (id, repo_id, created_at, actor, kind, entity_type, entity_id, correlation_id, payload)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        rusqlite::params![id, repo_id, now, actor, kind, entity_type, entity_id, correlation_id, serde_json::to_string(&payload)?],
    )?;
    Ok(id)
}

pub fn compute_repo_id(repo: &Path) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let canonical = repo.canonicalize().unwrap_or_else(|_| repo.to_path_buf());
    let mut hasher = DefaultHasher::new();
    canonical.to_string_lossy().hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}
