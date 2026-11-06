-- Private immutable plans and native receipts. No body bytes or bearer tokens.
CREATE TABLE trash_plans (
    operation_id TEXT PRIMARY KEY REFERENCES operations (operation_id),
    source_id TEXT NOT NULL CHECK (length(source_id) = 32),
    plan_hash TEXT NOT NULL CHECK (length(plan_hash) = 64),
    plan TEXT NOT NULL CHECK (length(CAST(plan AS BLOB)) BETWEEN 1 AND 16384)
) STRICT;

CREATE TABLE trash_members (
    operation_id TEXT NOT NULL REFERENCES trash_plans (operation_id),
    ordinal INTEGER NOT NULL CHECK (ordinal IN (0, 1)),
    status TEXT NOT NULL CHECK (status IN ('pending', 'intended', 'moved', 'missing', 'failed', 'uncertain')),
    receipt TEXT CHECK (receipt IS NULL OR length(CAST(receipt AS BLOB)) BETWEEN 1 AND 8192),
    reason TEXT CHECK (reason IS NULL OR reason IN ('unavailable', 'changed', 'unsafe_path', 'ownership_ambiguous', 'root_changed', 'uncertain')),
    checkpoint INTEGER NOT NULL CHECK (checkpoint >= 1),
    PRIMARY KEY (operation_id, ordinal),
    CHECK (status != 'moved' OR receipt IS NOT NULL),
    CHECK (status != 'missing' OR ordinal = 1)
) STRICT;

-- A fingerprint remains consumed even when a process dies before removing its
-- transient preview. The bearer credential is never written to SQLite.
CREATE TABLE trash_authorizations (
    fingerprint TEXT PRIMARY KEY CHECK (length(fingerprint) = 64),
    operation_id TEXT NOT NULL,
    step_id TEXT NOT NULL,
    plan_hash TEXT NOT NULL CHECK (length(plan_hash) = 64),
    FOREIGN KEY (operation_id, step_id) REFERENCES operation_steps (operation_id, step_id),
    UNIQUE (operation_id, step_id)
) STRICT;

CREATE INDEX operations_unfinished_trash ON operations (operation_id)
    WHERE kind = 'source_trash' AND state IN ('running', 'paused', 'uncertain');
