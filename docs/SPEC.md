# latch Spec

Status: Draft v0.1 for Nix/Bjarn review
Project: latch
Purpose: Project-scoped coordination ledger for collaborating agents

## Product Thesis

`latch` is a local-first coordination ledger for AI agents working together in one
repository or filesystem project. It persists the collaboration state that chat
alone loses: active file claims, decisions, handoff tasks, API contracts, and
known hazards.

The tool is not agent memory, not a chat system, and not a project manager. It is
the durable layer between ephemeral A2A negotiation and concrete file edits.

## Primary Users

- AI coding agents sharing a workspace.
- Humans who want to inspect coordination state without reading transcripts.
- Future host systems that want to inject compact collaboration context into an
  agent prompt.

## Core Problem

During multi-agent work, these facts must survive compaction and session
boundaries:

- who is actively touching which paths
- what decisions have already been agreed
- which tasks are waiting for which agent
- what contracts or schemas have been negotiated
- which commands or repo behaviors have surprising side effects

Without a persistent coordination layer, agents repeat negotiation, rediscover
hazards, and risk stepping on each other's files.

## Non-Goals

- No daemon or server in MVP.
- No web app in MVP.
- No cloud sync, accounts, or remote collaboration.
- No replacement for Git history.
- No long-term semantic memory.
- No arbitrary workflow engine.
- No hidden write behavior. Every command that mutates state must append an
  event.

## Storage Model

### Location

By default, `latch` stores project state under the repository or project root:

```text
.agent-workspace/workspace.sqlite
```

Project root resolution:

1. Use `--repo <path>` when provided.
2. Else use `git rev-parse --show-toplevel` when inside a Git repo.
3. Else use the current working directory.

`.agent-workspace/` should be gitignored by default. The ledger is coordination
state, not product source, unless a project explicitly chooses to commit it.

### SQLite

MVP uses SQLite with:

- WAL mode
- foreign keys enabled
- `busy_timeout` of at least 5000 ms
- migrations tracked in `schema_migrations`
- all writes in transactions

No daemon is required. SQLite locking is enough for the first version.

## Core Architecture

`latch` has two layers:

1. Append-only `events`
2. Materialized current-state tables rebuilt or updated from events

This preserves both current truth and provenance.

### Event Row

Every mutating command appends one event.

```json
{
  "id": "01JZ...ULID",
  "repo_id": "sha256-of-root-path-or-git-origin",
  "created_at": "2026-06-22T03:20:00Z",
  "actor": "bjarn",
  "kind": "claim.acquired",
  "entity_type": "claim",
  "entity_id": "01JZ...ULID",
  "correlation_id": null,
  "payload": {}
}
```

Required fields:

| Field | Description |
| --- | --- |
| `id` | Stable event id. ULID preferred for time-sortable ids. |
| `repo_id` | Stable id for the workspace. |
| `created_at` | UTC timestamp. |
| `actor` | Agent or human id responsible for the event. |
| `kind` | Dot-scoped event name. |
| `entity_type` | `claim`, `decision`, `task`, `contract`, `note`, or `workspace`. |
| `entity_id` | Id of the entity affected. |
| `correlation_id` | Optional id tying multi-step operations together. |
| `payload` | JSON payload for the event. |

## Entity Models

### Claims

A claim says an actor intends to work on a path for a bounded time.

Claim fields:

```json
{
  "id": "claim_01JZ...",
  "owner": "bjarn",
  "path": "frontend/app.js",
  "scope": "file",
  "intent": "validation summary polish",
  "status": "active",
  "acquired_at": "2026-06-22T03:20:00Z",
  "heartbeat_at": "2026-06-22T03:35:00Z",
  "expires_at": "2026-06-22T05:20:00Z",
  "released_at": null
}
```

Rules:

- `path` is project-relative, slash-normalized, and must not contain traversal.
- `scope` is `file` or `dir`.
- Claims may target paths that do not exist yet.
- Active, unexpired claims conflict by path containment:
  - `frontend/` conflicts with `frontend/app.js`.
  - `src/lib.rs` conflicts with `src/lib.rs`.
  - `frontend/app.js` does not conflict with `frontend/styles.css`.
- Expired claims do not block new claims.
- `renew` extends `expires_at` and updates `heartbeat_at`.
- `release` marks the claim released and appends an event.

### Decisions

A decision records an agreed architectural or process choice.

```json
{
  "id": "decision_01JZ...",
  "title": "Validation result changes are additive",
  "body": "Existing frontend fields remain stable; structured diagnostics append new fields.",
  "participants": ["bjarn", "nix"],
  "tags": ["frontend", "backend-contract"],
  "status": "active",
  "created_at": "2026-06-22T03:25:00Z",
  "superseded_by": null
}
```

Decisions are append-only. Editing a decision creates a new event. Replacing a
decision marks the old one as superseded.

### Contracts

A contract records a schema, API shape, file boundary, or behavior agreement.

```json
{
  "id": "contract_01JZ...",
  "name": "validation-result",
  "version": "v1",
  "format": "json",
  "body": {},
  "owner": "nix",
  "consumers": ["bjarn"],
  "status": "active",
  "created_at": "2026-06-22T03:30:00Z"
}
```

Rules:

- `name + version` is unique among active contracts.
- Contract bodies may be JSON or Markdown text.
- New versions should not mutate older versions.
- Contract bodies are stored in SQLite only for MVP. File export is a possible
  v2 feature.

### Tasks

A task is an async handoff item.

```json
{
  "id": "task_01JZ...",
  "title": "Send get_toolchain_status shape",
  "body": "Frontend may surface a readiness badge later.",
  "assigned_to": "nix",
  "created_by": "bjarn",
  "status": "open",
  "priority": "normal",
  "claim_refs": [],
  "created_at": "2026-06-22T03:35:00Z",
  "updated_at": "2026-06-22T03:35:00Z"
}
```

Statuses:

- `open`
- `taken`
- `done`
- `canceled`

### Notes

Notes capture useful coordination facts that are not tasks or decisions.

Kinds:

- `hazard`: a repo behavior that can surprise agents
- `handoff`: session handoff context
- `observation`: useful fact

Example hazard:

```json
{
  "kind": "hazard",
  "body": "cargo test mutates tracked src-tauri/target artifacts in this repo"
}
```

Notes are durable until explicitly removed. MVP does not include note TTLs.

## Command Contract

Global flags:

```text
--repo <path>          Project root override
--actor <id>          Actor id; default from LATCH_ACTOR, then system username
--format json|text    Output format; default json, except context defaults text
```

Exit codes:

| Code | Meaning |
| --- | --- |
| `0` | Success |
| `1` | Validation or usage error |
| `2` | Claim conflict |
| `3` | Entity not found |
| `4` | Storage or migration error |

### init

```sh
latch init
```

Creates `.agent-workspace/workspace.sqlite`, applies migrations, and writes a
`workspace.initialized` event.

### status

```sh
latch status
latch status --for bjarn
```

Returns current coordination state:

- active claims
- open or taken tasks assigned to the actor
- recent decisions
- recently changed contracts
- active hazards

Without `--for`, `status` is unfiltered and shows coordination state across all
actors. With `--for <actor>`, it filters actor-specific claims and tasks while
still including shared decisions, contracts, and hazards.

### context

```sh
latch context --for bjarn --format text
```

Returns a compact prompt-injection summary. This is an MVP feature because it
directly addresses compaction and cold-start coordination loss.

Unlike other commands, `context` defaults to text because its primary consumer is
an agent prompt, not a parser. Use `--format json` when a host system needs
structured context.

Text shape:

```text
Latch context for bjarn in learnRust:
- Active claims: frontend/app.js until 05:20Z (validation summary polish)
- Assigned tasks: review validation-result v2
- Recent decisions: Validation result changes are additive
- Contracts: validation-result v1 active
- Hazards: cargo test mutates tracked src-tauri/target artifacts
```

JSON shape:

```json
{
  "actor": "bjarn",
  "repo": "/home/mpfeifer/projects/learnRust",
  "active_claims": [],
  "assigned_tasks": [],
  "recent_decisions": [],
  "contracts": [],
  "hazards": []
}
```

### claims

```sh
latch claim acquire frontend/app.js --intent "validation summary polish" --ttl 2h
latch claim list
latch claim renew claim_01JZ... --ttl 2h
latch claim release claim_01JZ...
```

Conflict response:

```json
{
  "ok": false,
  "error": {
    "code": "claim_conflict",
    "message": "Path is already claimed",
    "conflicts": [
      {
        "id": "claim_01JZ...",
        "owner": "nix",
        "path": "frontend/",
        "expires_at": "2026-06-22T05:20:00Z"
      }
    ]
  }
}
```

### decisions

```sh
latch decision add --title "Validation result changes are additive" --body-file decision.md --tag frontend --tag backend-contract --participant bjarn --participant nix
latch decision list
latch decision show decision_01JZ...
latch decision supersede decision_01JZ... --title "Validation result v2" --body-file decision.md
```

### contracts

```sh
latch contract set validation-result v1 --format json --body-file validation-result.v1.json --consumer bjarn --owner nix
latch contract list
latch contract get validation-result v1
```

### tasks

```sh
latch task add --to nix --title "Send command shapes" --body "Frontend may surface them later"
latch task list --for nix
latch task take task_01JZ...
latch task done task_01JZ...
latch task cancel task_01JZ...
```

### notes

```sh
latch note add --kind hazard --body "cargo test mutates tracked target artifacts"
latch note list --kind hazard
```

### events

```sh
latch events list --since 2026-06-22T00:00:00Z
latch events show event_01JZ...
```

Events are useful for audits, debugging, and rebuilding views.

## MVP Implementation Plan

Language and libraries:

- Rust 2021 or newer
- `clap` for CLI parsing
- `rusqlite` or `sqlx` for SQLite
- `serde` and `serde_json` for output contracts
- `time` or `chrono` for timestamps and TTL parsing
- `ulid` or equivalent for sortable ids

Recommended module layout:

```text
src/
  main.rs
  cli.rs
  db.rs
  events.rs
  claims.rs
  decisions.rs
  contracts.rs
  tasks.rs
  notes.rs
  context.rs
  output.rs
tests/
  cli_claims.rs
  cli_context.rs
```

Build order:

1. CLI scaffold and `init`.
2. SQLite migrations and event append helper.
3. Claim acquire/list/renew/release with conflict tests.
4. Task add/list/take/done/cancel.
5. Decision add/list/show/supersede.
6. Contract set/list/get.
7. Note add/list.
8. Status and context aggregators.
9. README and examples.

## Test Plan

MVP integration tests should verify:

- `init` creates the DB and is idempotent.
- Claiming a file blocks another active claim on the same file.
- Claiming a directory blocks child file claims.
- Expired claims do not block new claims.
- Releasing a claim appends an event and removes it from active claims.
- Task lifecycle transitions are persisted.
- Decisions preserve participants and tags.
- Contracts preserve versioned bodies.
- `context --for <actor>` includes claims, tasks, decisions, contracts, and hazards.
- JSON output remains stable enough for snapshot tests.

## Open Questions

Resolved for MVP:

1. Actor fallback order is `--actor`, then `LATCH_ACTOR`, then system username.
2. `context` defaults to text; every other command defaults to JSON.
3. Notes do not expire automatically and are removable by id.
4. Contract bodies live only in SQLite.

Deferred:

1. Exporting contract bodies into `.agent-workspace/contracts/` for easier
   filesystem diffing.
2. Optional expiration for non-hazard notes.
