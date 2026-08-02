# Switchboard Integration

Canonical plan:

```text
/home/mpfeifer/projects/switchboard/docs/LATCH-INTEGRATION-PLAN.md
```

Latch's role in the integration is repo-local coordination truth:

- claims
- tasks
- decisions
- contracts
- hazards
- compact context

Switchboard's role is live human/agent collaboration:

- rooms
- DMs and groups
- typed events
- threads
- human UI
- agent CLI/HTTP/store access

The two tools should integrate through an adapter contract. Latch should remain
usable directly through:

```bash
latch --repo <repo> context --for <actor> --format json
```

Sandboxed agents must not need a daemon, browser, LAN bind, or harness-specific
bridge to use Latch coordination state.

Implemented Switchboard adapter path:

```bash
switchboard --transport store \
  project bind project.latch <repo> \
  --latch-bin <path-to-latch>

switchboard --transport store \
  project actor-map project.latch agent.bjarn bjarn

switchboard --transport store \
  project context project.latch \
  --for agent.bjarn

switchboard --transport store \
  latch promote evt_000000000123 \
  --as task \
  --to agent.helix \
  --by agent.bjarn
```

Switchboard stores bindings and promotion receipts in its own database. Latch
remains usable directly and is still the source of truth for claims, tasks,
decisions, contracts, hazards, and context.
