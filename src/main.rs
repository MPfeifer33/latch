mod claims;
mod cli;
mod context;
mod contracts;
mod db;
mod decisions;
mod events;
mod notes;
mod output;
mod tasks;

use agent_tools_core::{exit_with, ExitCode, RepoError};
use clap::Parser;
use cli::{Cli, Command};

fn main() {
    let cli = Cli::parse();
    let result = run(&cli);
    match result {
        Ok(()) => {}
        Err(e) => {
            e.report(cli.is_json());
            exit_with(e.exit_code());
        }
    }
}

fn run(cli: &Cli) -> Result<(), LatchError> {
    match &cli.command {
        Command::Init => {
            let repo = cli.resolve_repo()?;
            db::init_workspace(&repo)?;
            output::success_message("Workspace initialized", &repo, cli.is_json());
            Ok(())
        }
        Command::Status { r#for: actor } => {
            let repo = cli.resolve_repo()?;
            let conn = db::open_workspace(&repo)?;
            context::show_status(&conn, actor.as_deref(), cli.is_json())?;
            Ok(())
        }
        Command::Doctor {
            r#for: actor,
            strict,
        } => {
            let repo = cli.resolve_repo()?;
            let actor_resolved = actor.clone().or_else(|| Some(cli.resolve_actor()));
            let doctor = context::show_doctor(&repo, actor_resolved.as_deref(), cli.is_json())?;
            if *strict {
                exit_with(doctor.strict_exit_code());
            }
            Ok(())
        }
        Command::Context { r#for: actor } => {
            let repo = cli.resolve_repo()?;
            let conn = db::open_workspace(&repo)?;
            let actor_resolved = actor.clone().or_else(|| Some(cli.resolve_actor()));
            context::show_context(
                &conn,
                &repo,
                actor_resolved.as_deref(),
                cli.context_is_json(),
            )?;
            Ok(())
        }
        Command::Claim(cmd) => claims::handle(cmd, cli),
        Command::Decision(cmd) => decisions::handle(cmd, cli),
        Command::Contract(cmd) => contracts::handle(cmd, cli),
        Command::Task(cmd) => tasks::handle(cmd, cli),
        Command::Note(cmd) => notes::handle(cmd, cli),
        Command::Events(cmd) => events::handle_cmd(cmd, cli),
    }
}

#[derive(Debug, thiserror::Error)]
pub enum LatchError {
    #[error("{0}")]
    Validation(String),
    /// The path is held by active claims; `conflicts` carries their JSON rows
    /// so the JSON error report can include them in the one document it prints.
    #[error("claim conflict on {path}")]
    ClaimConflict {
        path: String,
        conflicts: Vec<serde_json::Value>,
    },
    #[error("not found: {0}")]
    NotFound(String),
    #[error("storage error: {0}")]
    Storage(String),
    #[error("database error: {0}")]
    Db(#[from] rusqlite::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

impl From<RepoError> for LatchError {
    fn from(err: RepoError) -> Self {
        LatchError::Io(err.into())
    }
}

impl LatchError {
    pub fn exit_code(&self) -> i32 {
        match self {
            LatchError::Validation(_) => ExitCode::Validation.code(),
            LatchError::ClaimConflict { .. } => ExitCode::ClaimConflict.code(),
            LatchError::NotFound(_) => ExitCode::NotFound.code(),
            LatchError::Storage(_) | LatchError::Db(_) | LatchError::Io(_) => {
                ExitCode::Storage.code()
            }
            LatchError::Json(_) => ExitCode::Validation.code(),
        }
    }

    pub fn error_code(&self) -> &'static str {
        match self {
            LatchError::Validation(_) => "validation_error",
            LatchError::ClaimConflict { .. } => "claim_conflict",
            LatchError::NotFound(_) => "not_found",
            LatchError::Storage(_) | LatchError::Db(_) => "storage_error",
            LatchError::Io(_) => "io_error",
            LatchError::Json(_) => "json_error",
        }
    }

    /// Print exactly one error report on stderr: the shared
    /// `{"ok": false, "error": {code, message}}` document in JSON mode (a claim
    /// conflict adds its `conflicts` rows to that same document, per SPEC.md),
    /// or `error: <message>` in text mode.
    pub fn report(&self, is_json: bool) {
        match self {
            LatchError::ClaimConflict { conflicts, .. } if is_json => {
                let mut value = agent_tools_core::error_value(self.error_code(), "Path is already claimed");
                value["error"]["conflicts"] = serde_json::Value::Array(conflicts.clone());
                eprintln!(
                    "{}",
                    serde_json::to_string_pretty(&value).unwrap_or_else(|_| value.to_string())
                );
            }
            _ => agent_tools_core::report_error(is_json, self.error_code(), &self.to_string()),
        }
    }
}
