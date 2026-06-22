# PROJECT.md — latch

**What:** Project-scoped coordination ledger for collaborating AI agents.

**Status:** Skeleton complete — compiles, CLI routes, DB layer with migrations, claims/tasks/notes fully implemented. Decisions, contracts, context/status are stubs awaiting Bjarn.

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
| decisions.rs | Bjarn | Stub |
| contracts.rs | Bjarn | Stub |
| context.rs | Bjarn | Stub |
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
- Exit codes: 0 success, 1 validation, 2 claim conflict, 3 not found, 4 storage error

## Last Updated

2026-06-21 — Initial skeleton commit (Nix)
