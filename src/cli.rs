use clap::{Parser, Subcommand};
use std::path::PathBuf;

use crate::LatchError;

#[derive(Parser, Debug)]
#[command(name = "latch", version, about = "Project-scoped coordination ledger for collaborating agents")]
pub struct Cli {
    /// Project root override
    #[arg(long, global = true)]
    pub repo: Option<PathBuf>,

    /// Actor id (overrides LATCH_ACTOR env and system username)
    #[arg(long, global = true)]
    pub actor: Option<String>,

    /// Output format
    #[arg(long, global = true)]
    pub format: Option<OutputFormat>,

    #[command(subcommand)]
    pub command: Command,
}

impl Cli {
    pub fn resolve_repo(&self) -> Result<PathBuf, LatchError> {
        if let Some(ref repo) = self.repo {
            return Ok(repo.clone());
        }
        // Try git root
        if let Ok(output) = std::process::Command::new("git")
            .args(["rev-parse", "--show-toplevel"])
            .output()
        {
            if output.status.success() {
                let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
                return Ok(PathBuf::from(path));
            }
        }
        // Fall back to cwd
        std::env::current_dir().map_err(|e| LatchError::Io(e))
    }

    pub fn resolve_actor(&self) -> String {
        if let Some(ref actor) = self.actor {
            return actor.clone();
        }
        if let Ok(actor) = std::env::var("LATCH_ACTOR") {
            return actor;
        }
        whoami::username()
    }

    pub fn is_json(&self) -> bool {
        !matches!(self.format, Some(OutputFormat::Text))
    }

    #[allow(dead_code)]
    pub fn context_is_json(&self) -> bool {
        matches!(self.format, Some(OutputFormat::Json))
    }
}

#[derive(Debug, Clone, clap::ValueEnum)]
pub enum OutputFormat {
    Json,
    Text,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Initialize the workspace ledger
    Init,

    /// Show coordination status
    Status {
        /// Filter to a specific actor
        #[arg(long)]
        r#for: Option<String>,
    },

    /// Get compact prompt-injection context
    Context {
        /// Actor to generate context for
        #[arg(long)]
        r#for: Option<String>,
    },

    /// Manage file/directory claims
    #[command(subcommand)]
    Claim(ClaimCommand),

    /// Manage architectural decisions
    #[command(subcommand)]
    Decision(DecisionCommand),

    /// Manage API/schema contracts
    #[command(subcommand)]
    Contract(ContractCommand),

    /// Manage async task handoffs
    #[command(subcommand)]
    Task(TaskCommand),

    /// Manage coordination notes and hazards
    #[command(subcommand)]
    Note(NoteCommand),

    /// Query the event log
    #[command(subcommand)]
    Events(EventsCommand),
}

#[derive(Subcommand, Debug)]
pub enum ClaimCommand {
    /// Acquire a claim on a path
    Acquire {
        /// Path to claim (project-relative)
        path: String,
        /// Purpose of the claim
        #[arg(long)]
        intent: Option<String>,
        /// Time-to-live (e.g., "2h", "30m")
        #[arg(long, default_value = "2h")]
        ttl: String,
    },
    /// List active claims
    List,
    /// Renew an existing claim
    Renew {
        /// Claim ID to renew
        id: String,
        /// New TTL
        #[arg(long, default_value = "2h")]
        ttl: String,
    },
    /// Release a claim
    Release {
        /// Claim ID to release
        id: String,
    },
}

#[derive(Subcommand, Debug)]
pub enum DecisionCommand {
    /// Record a new decision
    Add {
        #[arg(long)]
        title: String,
        #[arg(long)]
        body: Option<String>,
        #[arg(long = "body-file")]
        body_file: Option<PathBuf>,
        #[arg(long)]
        tag: Vec<String>,
        #[arg(long)]
        participant: Vec<String>,
    },
    /// List decisions
    List,
    /// Show a specific decision
    Show { id: String },
    /// Supersede a decision with a new one
    Supersede {
        /// ID of decision to supersede
        id: String,
        #[arg(long)]
        title: String,
        #[arg(long)]
        body: Option<String>,
        #[arg(long = "body-file")]
        body_file: Option<PathBuf>,
    },
}

#[derive(Subcommand, Debug)]
pub enum ContractCommand {
    /// Set or update a contract
    Set {
        /// Contract name
        name: String,
        /// Contract version
        version: String,
        #[arg(long)]
        format: Option<String>,
        #[arg(long)]
        body: Option<String>,
        #[arg(long = "body-file")]
        body_file: Option<PathBuf>,
        #[arg(long)]
        owner: Option<String>,
        #[arg(long)]
        consumer: Vec<String>,
    },
    /// List contracts
    List,
    /// Get a specific contract
    Get {
        name: String,
        version: Option<String>,
    },
}

#[derive(Subcommand, Debug)]
pub enum TaskCommand {
    /// Add a new task
    Add {
        #[arg(long)]
        to: String,
        #[arg(long)]
        title: String,
        #[arg(long)]
        body: Option<String>,
        #[arg(long)]
        priority: Option<String>,
    },
    /// List tasks
    List {
        /// Filter by assignee
        #[arg(long)]
        r#for: Option<String>,
    },
    /// Take a task (mark as in-progress)
    Take { id: String },
    /// Complete a task
    Done { id: String },
    /// Cancel a task
    Cancel { id: String },
}

#[derive(Subcommand, Debug)]
pub enum NoteCommand {
    /// Add a note
    Add {
        #[arg(long)]
        kind: String,
        #[arg(long)]
        body: String,
    },
    /// List notes
    List {
        /// Filter by kind
        #[arg(long)]
        kind: Option<String>,
    },
    /// Remove a note
    Remove { id: String },
}

#[derive(Subcommand, Debug)]
pub enum EventsCommand {
    /// List events
    List {
        /// Show events since this timestamp
        #[arg(long)]
        since: Option<String>,
        /// Limit number of events
        #[arg(long, default_value = "50")]
        limit: usize,
    },
    /// Show a specific event
    Show { id: String },
}
