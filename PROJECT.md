# PROJECT.md — latch

**What:** Project-scoped coordination ledger for collaborating AI agents.

**Status:** MVP surfaces implemented — compiles, CLI routes, DB layer with migrations, claims/tasks/notes/events, decisions/contracts, and status/context/doctor aggregators are implemented. Full integration suite passes.

**Tech:** Rust 2021, clap 4, rusqlite (bundled SQLite), serde/serde_json, chrono, ulid, thiserror, whoami.

**Storage:** `.agent-workspace/workspace.sqlite` under repo root. WAL mode, append-only event log + materialized state tables.

## Module Ownership

| Module | Owner | Status |
|--------|-------|--------|
| cli.rs | Nix | Done |
| db.rs | Nix | Done |
| events.rs | Nix | Done |
| claims.rs | Nix | Done |
| tasks.rs | Nix | Done |
| notes.rs | Nix | Done |
| decisions.rs | Bjarn | Done |
| contracts.rs | Bjarn | Done |
| context.rs | Bjarn | Done |
| output.rs | Shared | Minimal |

## Build

```sh
cargo build
cargo check
cargo test
```

## Key Design Choices

- Append-only event log for provenance (every mutation emits an event)
- Claims use TTL + heartbeat pattern with path-containment conflict detection
- ULID for time-sortable IDs
- Actor resolution: --actor flag > LATCH_ACTOR env > system username
- Agent preflight: `latch doctor` reports workspace readiness, coordination gates, and stable recommended commands
- Exit codes: 0 success, 1 validation, 2 claim conflict, 3 not found, 4 storage error; `doctor --strict` uses gate exits 10/20 after printing `ok: true`

## Last Updated

2026-08-06 — Added agent-first `doctor` preflight; `cargo test` passes with 24 integration tests.
