mod cli;
mod db;
mod events;
mod claims;
mod tasks;
mod notes;
mod decisions;
mod contracts;
mod context;
mod output;

use clap::Parser;
use cli::{Cli, Command};

fn main() {
    let cli = Cli::parse();
    let result = run(&cli);
    match result {
        Ok(()) => {}
        Err(e) => {
            let code = e.exit_code();
            if cli.is_json() {
                let err_json = serde_json::json!({
                    "ok": false,
                    "error": {
                        "code": e.error_code(),
                        "message": e.to_string(),
                    }
                });
                eprintln!("{}", serde_json::to_string_pretty(&err_json).unwrap());
            } else {
                eprintln!("error: {e}");
            }
            std::process::exit(code);
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
        Command::Context { r#for: actor } => {
            let repo = cli.resolve_repo()?;
            let conn = db::open_workspace(&repo)?;
            let actor_resolved = actor.clone().or_else(|| Some(cli.resolve_actor()));
            context::show_context(&conn, actor_resolved.as_deref(), cli.is_json())?;
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
    #[error("claim conflict: {0}")]
    ClaimConflict(String),
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

impl LatchError {
    pub fn exit_code(&self) -> i32 {
        match self {
            LatchError::Validation(_) => 1,
            LatchError::ClaimConflict(_) => 2,
            LatchError::NotFound(_) => 3,
            LatchError::Storage(_) | LatchError::Db(_) | LatchError::Io(_) => 4,
            LatchError::Json(_) => 1,
        }
    }

    pub fn error_code(&self) -> &'static str {
        match self {
            LatchError::Validation(_) => "validation_error",
            LatchError::ClaimConflict(_) => "claim_conflict",
            LatchError::NotFound(_) => "not_found",
            LatchError::Storage(_) | LatchError::Db(_) => "storage_error",
            LatchError::Io(_) => "io_error",
            LatchError::Json(_) => "json_error",
        }
    }
}
