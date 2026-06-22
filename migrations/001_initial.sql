-- Initial schema for latch coordination ledger

CREATE TABLE events (
    id TEXT PRIMARY KEY,
    repo_id TEXT NOT NULL,
    created_at TEXT NOT NULL,
    actor TEXT NOT NULL,
    kind TEXT NOT NULL,
    entity_type TEXT NOT NULL,
    entity_id TEXT NOT NULL,
    correlation_id TEXT,
    payload TEXT NOT NULL DEFAULT '{}'
);

CREATE INDEX idx_events_created_at ON events(created_at);
CREATE INDEX idx_events_entity ON events(entity_type, entity_id);
CREATE INDEX idx_events_actor ON events(actor);

CREATE TABLE claims (
    id TEXT PRIMARY KEY,
    owner TEXT NOT NULL,
    path TEXT NOT NULL,
    scope TEXT NOT NULL CHECK (scope IN ('file', 'dir')),
    intent TEXT NOT NULL DEFAULT '',
    status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'released')),
    acquired_at TEXT NOT NULL,
    heartbeat_at TEXT NOT NULL,
    expires_at TEXT NOT NULL,
    released_at TEXT
);

CREATE INDEX idx_claims_status ON claims(status);
CREATE INDEX idx_claims_path ON claims(path);

CREATE TABLE decisions (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    body TEXT NOT NULL DEFAULT '',
    participants TEXT NOT NULL DEFAULT '[]',
    tags TEXT NOT NULL DEFAULT '[]',
    status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'superseded')),
    created_at TEXT NOT NULL,
    superseded_by TEXT
);

CREATE TABLE contracts (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    version TEXT NOT NULL,
    format TEXT NOT NULL DEFAULT 'json',
    body TEXT NOT NULL DEFAULT '',
    owner TEXT NOT NULL DEFAULT '',
    consumers TEXT NOT NULL DEFAULT '[]',
    status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'retired')),
    created_at TEXT NOT NULL,
    UNIQUE(name, version)
);

CREATE TABLE tasks (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    body TEXT NOT NULL DEFAULT '',
    assigned_to TEXT NOT NULL,
    created_by TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'open' CHECK (status IN ('open', 'taken', 'done', 'canceled')),
    priority TEXT NOT NULL DEFAULT 'normal',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX idx_tasks_assigned ON tasks(assigned_to, status);

CREATE TABLE notes (
    id TEXT PRIMARY KEY,
    kind TEXT NOT NULL CHECK (kind IN ('hazard', 'handoff', 'observation')),
    body TEXT NOT NULL,
    author TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE INDEX idx_notes_kind ON notes(kind);
