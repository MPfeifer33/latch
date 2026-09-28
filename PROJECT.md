# PROJECT.md — latch

**What:** Project-scoped coordination ledger for collaborating AI agents.

**Status:** MVP surfaces implemented — compiles, CLI routes, DB layer with migrations, claims/tasks/notes/events, decisions/contracts, and status/context/doctor aggregators are implemented. Full integration suite passes (29 tests). Shared plumbing (repo resolution, `--format`, exit codes, error report) now comes from `agent-tools-core`.



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

Depends on [`agent-tools-core`](https://github.com/MPfeifer33/agent-tools-core) by git tag (`v0.1.0`); standalone clones build without anything beside them.

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
- Repo resolution (agent-tools-core): --repo flag > AGENT_REPO env > nearest `.git` ancestor of cwd > cwd; existing paths are canonicalized
- Errors print exactly one document on stderr; a claim conflict carries its `conflicts[]` inside that document
- Agent preflight: `latch doctor` reports workspace readiness, coordination gates, and stable recommended commands
- Exit codes (shared table in agent-tools-core): 0 success, 1 validation, 2 claim conflict, 3 not found, 4 storage error; `doctor --strict` uses gate exits 10/20 after printing `ok: true`

## Last Updated

2026-09-28: README gained an "In ten seconds" block with a real invocation and its output above the fold.

2026-09-11 — Moved repo resolution, `--format`, exit codes, and the stderr error report onto `agent-tools-core`; a claim conflict now prints exactly one JSON document (with `conflicts[]`, exit 2 unchanged); added `--version` and conflict tests. `cargo test` passes with 29 integration tests.

2026-08-06 — Added agent-first `doctor` preflight; `cargo test` passes with 24 integration tests.
