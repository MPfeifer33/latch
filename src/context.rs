use chrono::Utc;
use rusqlite::{types::ToSql, Connection};
use serde::Serialize;
use serde_json::{json, Value};
use std::path::Path;

use crate::{db, output, LatchError};

const DOCTOR_SCHEMA_VERSION: &str = "latch.doctor.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DoctorActionLevel {
    None,
    Initialize,
    Coordinate,
    Review,
}

impl DoctorActionLevel {
    pub fn strict_exit_code(self) -> i32 {
        match self {
            DoctorActionLevel::None => 0,
            DoctorActionLevel::Initialize | DoctorActionLevel::Coordinate => 10,
            DoctorActionLevel::Review => 20,
        }
    }

    fn label(self) -> &'static str {
        match self {
            DoctorActionLevel::None => "none",
            DoctorActionLevel::Initialize => "initialize",
            DoctorActionLevel::Coordinate => "coordinate",
            DoctorActionLevel::Review => "review",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DoctorStatus {
    Ready,
    Caution,
}

impl DoctorStatus {
    fn label(self) -> &'static str {
        match self {
            DoctorStatus::Ready => "ready",
            DoctorStatus::Caution => "caution",
        }
    }
}

#[derive(Debug, Serialize)]
pub struct LatchDoctor {
    pub schema_version: String,
    pub status: DoctorStatus,
    pub action_level: DoctorActionLevel,
    pub actor: Option<String>,
    pub repo: String,
    pub workspace: WorkspaceDoctor,
    pub gates: DoctorGates,
    pub counts: DoctorCounts,
    pub advice: String,
    pub recommendations: Vec<String>,
    pub recommended_commands: Vec<RecommendedCommand>,
}

impl LatchDoctor {
    pub fn strict_exit_code(&self) -> i32 {
        self.action_level.strict_exit_code()
    }
}

#[derive(Debug, Serialize)]
pub struct WorkspaceDoctor {
    pub initialized: bool,
    pub path: String,
}

#[derive(Debug, Serialize, Default)]
pub struct DoctorGates {
    pub workspace_initialized: bool,
    pub active_claims: bool,
    pub assigned_tasks: bool,
    pub active_contracts: bool,
    pub active_hazards: bool,
}

#[derive(Debug, Serialize, Default)]
pub struct DoctorCounts {
    pub active_claims: usize,
    pub assigned_tasks: usize,
    pub recent_decisions: usize,
    pub active_contracts: usize,
    pub active_hazards: usize,
}

#[derive(Debug, Serialize)]
pub struct RecommendedCommand {
    pub kind: RecommendationKind,
    pub command: Option<String>,
    pub argv: Option<Vec<String>>,
    pub label: String,
    pub reason: String,
    pub reason_code: String,
    pub required: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RecommendationKind {
    Command,
    Manual,
}

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

pub fn show_doctor(
    repo: &Path,
    actor: Option<&str>,
    is_json: bool,
) -> Result<LatchDoctor, LatchError> {
    let workspace_path = db::workspace_path(repo);

    let doctor = if workspace_path.exists() {
        let conn = db::open_workspace(repo)?;
        let claims = active_claims(&conn, actor)?;
        let tasks = active_tasks(&conn, actor)?;
        let decisions = recent_decisions(&conn, 8)?;
        let contracts = active_contracts(&conn, 8)?;
        let hazards = hazards(&conn, 8)?;
        build_doctor(
            repo,
            actor,
            true,
            claims.len(),
            tasks.len(),
            decisions.len(),
            contracts.len(),
            hazards.len(),
        )
    } else {
        build_doctor(repo, actor, false, 0, 0, 0, 0, 0)
    };

    if is_json {
        output::print_json_value(json!({
            "ok": true,
            "schema_version": &doctor.schema_version,
            "status": doctor.status,
            "action_level": doctor.action_level,
            "doctor": &doctor,
            "workspace": &doctor.workspace,
            "gates": &doctor.gates,
            "counts": &doctor.counts,
            "recommendations": &doctor.recommendations,
            "recommended_commands": &doctor.recommended_commands,
        }))?;
    } else {
        print_doctor_text(&doctor);
    }

    Ok(doctor)
}

#[allow(clippy::too_many_arguments)]
fn build_doctor(
    repo: &Path,
    actor: Option<&str>,
    workspace_initialized: bool,
    active_claims: usize,
    assigned_tasks: usize,
    recent_decisions: usize,
    active_contracts: usize,
    active_hazards: usize,
) -> LatchDoctor {
    let gates = DoctorGates {
        workspace_initialized,
        active_claims: active_claims > 0,
        assigned_tasks: assigned_tasks > 0,
        active_contracts: active_contracts > 0,
        active_hazards: active_hazards > 0,
    };
    let counts = DoctorCounts {
        active_claims,
        assigned_tasks,
        recent_decisions,
        active_contracts,
        active_hazards,
    };
    let action_level = action_level_for(&gates);
    let status = status_for(action_level);
    let recommended_commands = recommended_commands_for(action_level, &gates);
    let recommendations = recommended_commands
        .iter()
        .map(recommendation_label)
        .collect();

    LatchDoctor {
        schema_version: DOCTOR_SCHEMA_VERSION.to_string(),
        status,
        action_level,
        actor: actor.map(ToOwned::to_owned),
        repo: repo.display().to_string(),
        workspace: WorkspaceDoctor {
            initialized: workspace_initialized,
            path: db::workspace_path(repo).display().to_string(),
        },
        gates,
        counts,
        advice: advice_for(action_level),
        recommendations,
        recommended_commands,
    }
}

fn action_level_for(gates: &DoctorGates) -> DoctorActionLevel {
    if !gates.workspace_initialized {
        DoctorActionLevel::Initialize
    } else if gates.active_hazards {
        DoctorActionLevel::Review
    } else if gates.assigned_tasks || gates.active_claims {
        DoctorActionLevel::Coordinate
    } else {
        DoctorActionLevel::None
    }
}

fn status_for(action_level: DoctorActionLevel) -> DoctorStatus {
    match action_level {
        DoctorActionLevel::None => DoctorStatus::Ready,
        DoctorActionLevel::Initialize
        | DoctorActionLevel::Coordinate
        | DoctorActionLevel::Review => DoctorStatus::Caution,
    }
}

fn recommended_commands_for(
    action_level: DoctorActionLevel,
    gates: &DoctorGates,
) -> Vec<RecommendedCommand> {
    let mut commands = Vec::new();

    if !gates.workspace_initialized {
        commands.push(command_recommendation(
            "latch init",
            &["latch", "init"],
            "Initialize the project coordination ledger",
            "workspace_missing",
            "no .agent-workspace/workspace.sqlite exists for this repo",
            true,
        ));
    }

    if gates.active_hazards {
        commands.push(command_recommendation(
            "latch note list --kind hazard",
            &["latch", "note", "list", "--kind", "hazard"],
            "Read active hazards before proceeding",
            "active_hazards",
            "one or more active hazards is recorded in the ledger",
            true,
        ));
    }

    if gates.assigned_tasks {
        commands.push(command_recommendation(
            "latch task list --for <actor>",
            &["latch", "task", "list", "--for", "<actor>"],
            "Review assigned Latch tasks",
            "assigned_tasks",
            "one or more open or taken tasks is assigned to this actor",
            false,
        ));
    }

    if gates.active_claims {
        commands.push(command_recommendation(
            "latch claim list",
            &["latch", "claim", "list"],
            "Review active file claims",
            "active_claims",
            "one or more active file claims is present for this actor or workspace",
            false,
        ));
    }

    if commands.is_empty() && action_level == DoctorActionLevel::None {
        commands.push(manual_recommendation(
            "No Latch-specific action required",
            "ready",
            "workspace is initialized and no active coordination gates are raised",
            false,
        ));
    }

    commands
}

fn command_recommendation(
    command: &str,
    argv: &[&str],
    label: &str,
    reason_code: &str,
    reason: &str,
    required: bool,
) -> RecommendedCommand {
    RecommendedCommand {
        kind: RecommendationKind::Command,
        command: Some(command.to_string()),
        argv: Some(argv.iter().map(|part| part.to_string()).collect()),
        label: label.to_string(),
        reason: reason.to_string(),
        reason_code: reason_code.to_string(),
        required,
    }
}

fn manual_recommendation(
    label: &str,
    reason_code: &str,
    reason: &str,
    required: bool,
) -> RecommendedCommand {
    RecommendedCommand {
        kind: RecommendationKind::Manual,
        command: None,
        argv: None,
        label: label.to_string(),
        reason: reason.to_string(),
        reason_code: reason_code.to_string(),
        required,
    }
}

fn recommendation_label(recommendation: &RecommendedCommand) -> String {
    recommendation
        .command
        .clone()
        .unwrap_or_else(|| recommendation.label.clone())
}

fn advice_for(action_level: DoctorActionLevel) -> String {
    match action_level {
        DoctorActionLevel::None => "workspace is ready; use normal coordination flow".to_string(),
        DoctorActionLevel::Initialize => {
            "workspace ledger is missing; run `latch init` before recording coordination state"
                .to_string()
        }
        DoctorActionLevel::Coordinate => {
            "coordination state needs attention before or during the next work pass".to_string()
        }
        DoctorActionLevel::Review => {
            "active hazards are present; read them before proceeding automatically".to_string()
        }
    }
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

fn print_doctor_text(doctor: &LatchDoctor) {
    println!(
        "latch doctor: {} ({})",
        doctor.status.label(),
        doctor.action_level.label()
    );
    println!();
    println!("  Repo: {}", doctor.repo);
    println!("  Actor: {}", doctor.actor.as_deref().unwrap_or("all"));
    println!("  Workspace initialized: {}", doctor.workspace.initialized);
    println!("  Workspace path: {}", doctor.workspace.path);
    println!();
    println!("  Gates:");
    println!(
        "    workspace_initialized: {}",
        doctor.gates.workspace_initialized
    );
    println!("    active_claims: {}", doctor.gates.active_claims);
    println!("    assigned_tasks: {}", doctor.gates.assigned_tasks);
    println!("    active_contracts: {}", doctor.gates.active_contracts);
    println!("    active_hazards: {}", doctor.gates.active_hazards);
    println!();
    println!("  Counts:");
    println!("    active_claims: {}", doctor.counts.active_claims);
    println!("    assigned_tasks: {}", doctor.counts.assigned_tasks);
    println!("    recent_decisions: {}", doctor.counts.recent_decisions);
    println!("    active_contracts: {}", doctor.counts.active_contracts);
    println!("    active_hazards: {}", doctor.counts.active_hazards);
    println!();
    println!("  Advice: {}", doctor.advice);
    println!();
    println!("  Recommended next commands:");
    for recommendation in &doctor.recommended_commands {
        let required = if recommendation.required {
            "required"
        } else {
            "optional"
        };
        match recommendation.kind {
            RecommendationKind::Command => println!(
                "    {} [{}] - {} ({})",
                recommendation.command.as_deref().unwrap_or(""),
                required,
                recommendation.label,
                recommendation.reason_code
            ),
            RecommendationKind::Manual => println!(
                "    {} [{}] - {}",
                recommendation.label, required, recommendation.reason_code
            ),
        }
    }
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
