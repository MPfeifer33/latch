# latch

`latch` is a project-scoped coordination ledger for AI agents sharing a
workspace. It persists the collaboration facts that should survive chat
history loss: path claims, architectural decisions, handoff tasks, contracts,
and repo hazards.

The core idea is simple: every mutation appends an event, and current state is
stored in small SQLite tables under the project itself.

## Suite Context

Latch is part of a local-first agent tool suite centered on
[Switchboard](https://github.com/MPfeifer33/switchboard):

- [Probe](https://github.com/MPfeifer33/probe): project preflight and drift
  scanner
- [Latch](https://github.com/MPfeifer33/latch): repo-local coordination ledger
- [Atlas](https://github.com/MPfeifer33/atlas): codebase graph and impact map
- [Sentinel](https://github.com/MPfeifer33/sentinel): regression risk watcher
- [Witness](https://github.com/MPfeifer33/witness): reproducible command
  evidence recorder

## Quickstart

```sh
cargo build

# Initialize coordination state for the current repo.
cargo run -- init

# Tell latch who is speaking.
export LATCH_ACTOR=builder

# Claim files while you work.
cargo run -- claim acquire src/context.rs --intent "context aggregator" --ttl 2h

# Record shared decisions and contracts.
cargo run -- decision add --title "Context defaults to text" --body "Prompt injection is the first consumer."
cargo run -- contract set validation-result v1 --body '{"success":true}' --consumer reviewer

# See the coordination picture.
cargo run -- status
cargo run -- doctor --for builder --format json
cargo run -- context --for builder
```

After installation, replace `cargo run --` with `latch`.

Install the CLI from a local checkout:

```sh
cargo install --path .
latch --help
```

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
latch --actor reviewer task add --to builder --title "Review context output"
```

## Output

Most commands default to JSON for machine consumption:

```sh
latch status --format json
```

`latch context` is the one exception. It defaults to compact text because its
primary use is prompt injection after an agent cold start:

```sh
latch context --for builder
latch context --for builder --format json
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
latch task add --to builder --title "Wire context status" --priority high
latch task list
latch task list --for builder
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
  --participant builder \
  --participant reviewer

latch decision list
latch decision show <decision-id>
latch decision supersede <decision-id> --title "Validation result v2" --body-file decision.md
```

### Contracts

Contracts record negotiated API shapes, schemas, behavior boundaries, or file
ownership agreements.

```sh
latch contract set validation-result v1 \
  --body-format json \
  --body-file validation-result.v1.json \
  --consumer builder \
  --owner reviewer

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
latch note add "Build takes 3 minutes on first run"
latch note add --kind hazard --body "cargo test can dirty tracked target artifacts"
latch note list
latch note list --kind hazard
latch note remove <note-id>
```

Notes are durable until explicitly removed.

### Status And Context

`doctor` is the agent preflight. It reports whether the workspace ledger exists,
whether coordination state needs attention, and which next command or manual
action an agent should take:

```sh
latch doctor --for builder --format json
latch doctor --for builder --strict --format json
```

JSON output includes `schema_version: latch.doctor.v1`, `status`,
`action_level`, `gates`, `counts`, and `recommended_commands`. Strict mode
prints the same report and exits by gate severity: `0` for `none`, `10` for
`initialize` or `coordinate`, and `20` for `review`.

`status` is an operational view for humans and agents:

```sh
latch status
latch status --for builder
```

Without `--for`, it shows all active coordination state. With `--for`, it
filters claims and tasks to that actor while still showing shared decisions,
contracts, and hazards.

`context` is a compact handoff view:

```sh
latch context --for builder
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
Switchboard integration notes are in [docs/SWITCHBOARD-INTEGRATION.md](docs/SWITCHBOARD-INTEGRATION.md);
the canonical cross-project plan lives at
`/path/to/projects/switchboard/docs/LATCH-INTEGRATION-PLAN.md`.

Important MVP choices:

- repo-local SQLite, no daemon
- append-only event log plus materialized views
- ULIDs for time-sortable IDs
- TTL-based claims with explicit renew/release
- durable hazards until explicit removal
- JSON-first CLI output except for prompt-oriented `context`

## License

Licensed under the Apache License, Version 2.0. See [LICENSE](LICENSE) and
[NOTICE](NOTICE). Redistributed or derivative works must preserve the NOTICE
attribution required by the license.
