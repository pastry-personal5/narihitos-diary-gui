CREATE TABLE schema_migrations (
    version INTEGER PRIMARY KEY,
    checksum TEXT NOT NULL
) STRICT;

CREATE TABLE workspace_meta (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    workspace_id TEXT NOT NULL
) STRICT;

CREATE TABLE operations (
    operation_id TEXT PRIMARY KEY CHECK (length(operation_id) = 36),
    kind TEXT NOT NULL CHECK (length(kind) BETWEEN 1 AND 64),
    hash_version INTEGER NOT NULL,
    input_hash TEXT NOT NULL CHECK (length(input_hash) = 64),
    state TEXT NOT NULL CHECK (state IN ('running', 'paused', 'uncertain', 'succeeded', 'failed', 'cancelled')),
    checkpoint INTEGER NOT NULL DEFAULT 0,
    result TEXT CHECK (result IS NULL OR length(result) <= 4096),
    workspace_id TEXT NOT NULL,
    root TEXT NOT NULL,
    native TEXT NOT NULL,
    adapter_version INTEGER NOT NULL
) STRICT;

-- Rows exist only while an operation is unfinished: they are the durable reservations.
CREATE TABLE operation_resources (
    operation_id TEXT NOT NULL REFERENCES operations (operation_id),
    resource_key TEXT NOT NULL,
    PRIMARY KEY (operation_id, resource_key)
) STRICT;
CREATE INDEX operation_resources_key ON operation_resources (resource_key);

CREATE TABLE operation_steps (
    operation_id TEXT NOT NULL REFERENCES operations (operation_id),
    step_id TEXT NOT NULL,
    input_hash TEXT NOT NULL CHECK (length(input_hash) = 64),
    expected_checkpoint INTEGER NOT NULL,
    new_checkpoint INTEGER NOT NULL,
    outcome TEXT NOT NULL CHECK (length(outcome) <= 1024),
    PRIMARY KEY (operation_id, step_id)
) STRICT;
