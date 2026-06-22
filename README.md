# latch

`latch` is a project-scoped coordination ledger for AI agents sharing a
workspace. It persists the collaboration facts that should survive chat
history loss: path claims, architectural decisions, handoff tasks, contracts,
and repo hazards.

The core idea is simple: every mutation appends an event, and current state is
stored in small SQLite tables under the project itself.

## Quickstart

```sh
cargo build

# Initialize coordination state for the current repo.
cargo run -- init

# Tell latch who is speaking.
export LATCH_ACTOR=bjarn

# Claim files while you work.
cargo run -- claim acquire src/context.rs --intent "context aggregator" --ttl 2h

# Record shared decisions and contracts.
cargo run -- decision add --title "Context defaults to text" --body "Prompt injection is the first consumer."
cargo run -- contract set validation-result v1 --body '{"success":true}' --consumer nix

# See the coordination picture.
cargo run -- status
cargo run -- context --for bjarn
```

After installation, replace `cargo run --` with `latch`.

## Storage

`latch init` creates:

```text
.agent-workspace/
  .gitignore
  workspace.sqlite
```

The workspace directory is ignored by default. It is coordination state, not a
product artifact.

SQLite runs in WAL mode with a short busy timeout, so multiple agents can use
the CLI without a daemon.

## Actor Resolution

Commands resolve the actor in this order:

1. `--actor <name>`
2. `LATCH_ACTOR`
3. the system username

Example:

```sh
latch --actor nix task add --to bjarn --title "Review context output"
```

## Output

Most commands default to JSON for machine consumption:

```sh
latch status --format json
```

`latch context` is the one exception. It defaults to compact text because its
primary use is prompt injection after an agent cold start:

```sh
latch context --for bjarn
latch context --for bjarn --format json
```

## Commands

### Claims

Claims protect active work areas with a TTL and path-containment conflict
checks.

```sh
latch claim acquire frontend/ --intent "UI pass" --ttl 2h
latch claim list
latch claim renew <claim-id> --ttl 4h
latch claim release <claim-id>
```

Claiming `src/` conflicts with another active claim on `src/main.rs`, and the
reverse is also true.

### Tasks

Tasks are async handoffs between agents.

```sh
latch task add --to bjarn --title "Wire context status" --priority high
latch task list
latch task list --for bjarn
latch task take <task-id>
latch task done <task-id>
latch task cancel <task-id>
```

### Decisions

Decisions record architecture or process choices. They are not edited in
place; replacing a decision supersedes the old one.

```sh
latch decision add \
  --title "Validation result changes are additive" \
  --body-file decision.md \
  --tag backend-contract \
  --participant bjarn \
  --participant nix

latch decision list
latch decision show <decision-id>
latch decision supersede <decision-id> --title "Validation result v2" --body-file decision.md
```

### Contracts

Contracts record negotiated API shapes, schemas, behavior boundaries, or file
ownership agreements.

```sh
latch contract set validation-result v1 \
  --body-file validation-result.v1.json \
  --consumer bjarn \
  --owner nix

latch contract list
latch contract get validation-result v1
```

Contract bodies are stored in SQLite for the MVP. JSON contract bodies are
validated on write.

### Notes

Notes capture useful coordination facts that are not tasks or decisions.

Valid kinds:

- `hazard`
- `handoff`
- `observation`

```sh
latch note add --kind hazard --body "cargo test can dirty tracked target artifacts"
latch note list
latch note list --kind hazard
latch note remove <note-id>
```

Notes are durable until explicitly removed.

### Status And Context

`status` is an operational view for humans and agents:

```sh
latch status
latch status --for bjarn
```

Without `--for`, it shows all active coordination state. With `--for`, it
filters claims and tasks to that actor while still showing shared decisions,
contracts, and hazards.

`context` is a compact handoff view:

```sh
latch context --for bjarn
```

It includes:

- active claims
- assigned open/taken tasks
- recent decisions
- active contracts
- durable hazards

### Events

Every mutation appends an event for provenance.

```sh
latch events list --limit 50
latch events show <event-id>
```

## Exit Codes

| Code | Meaning |
| ---- | ------- |
| `0` | Success |
| `1` | Validation or JSON error |
| `2` | Claim conflict |
| `3` | Not found |
| `4` | Storage, database, or IO error |

## Design Notes

The full design draft is in [docs/SPEC.md](docs/SPEC.md).

Important MVP choices:

- repo-local SQLite, no daemon
- append-only event log plus materialized views
- ULIDs for time-sortable IDs
- TTL-based claims with explicit renew/release
- durable hazards until explicit removal
- JSON-first CLI output except for prompt-oriented `context`
