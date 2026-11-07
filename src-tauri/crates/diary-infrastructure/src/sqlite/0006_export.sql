-- Destination-neutral export coordination. The intent (revision, digests, versions,
-- destination) is committed before any destination call. The snapshot holds diary text only
-- while the operation is unfinished; it is cleared by every terminal transition. No bearer
-- tokens are stored.
CREATE TABLE export_operations (
    operation_id TEXT PRIMARY KEY REFERENCES operations (operation_id),
    target TEXT NOT NULL CHECK (target IN ('icloud', 'notion')),
    destination_key TEXT NOT NULL CHECK (length(destination_key) = 64),
    authentic_id TEXT NOT NULL CHECK (length(authentic_id) BETWEEN 1 AND 64),
    date TEXT NOT NULL CHECK (length(date) = 10),
    revision_sequence INTEGER NOT NULL CHECK (revision_sequence >= 0),
    revision_sha TEXT NOT NULL CHECK (length(revision_sha) = 64),
    snapshot_digest TEXT NOT NULL CHECK (length(snapshot_digest) = 64),
    snapshot BLOB CHECK (snapshot IS NULL OR length(snapshot) <= 1114112),
    renderer_version INTEGER NOT NULL,
    api_version INTEGER NOT NULL,
    reason TEXT CHECK (reason IS NULL OR length(reason) <= 32)
) STRICT;

-- The last verified publication of one authentic file to one destination.
CREATE TABLE export_receipts (
    destination_key TEXT NOT NULL CHECK (length(destination_key) = 64),
    authentic_id TEXT NOT NULL CHECK (length(authentic_id) BETWEEN 1 AND 64),
    date TEXT NOT NULL CHECK (length(date) = 10),
    revision_sha TEXT NOT NULL CHECK (length(revision_sha) = 64),
    projection_digest TEXT NOT NULL CHECK (length(projection_digest) = 64),
    remote_ids TEXT NOT NULL CHECK (length(remote_ids) <= 1024),
    remote_fingerprint TEXT NOT NULL CHECK (length(remote_fingerprint) <= 256),
    PRIMARY KEY (destination_key, authentic_id)
) STRICT;

CREATE TABLE export_checkpoints (
    operation_id TEXT NOT NULL REFERENCES export_operations (operation_id),
    chunk INTEGER NOT NULL CHECK (chunk >= 0),
    digest TEXT NOT NULL CHECK (length(digest) = 64),
    status TEXT NOT NULL CHECK (status IN ('pending', 'applying', 'done', 'failed', 'unknown')),
    remote_id TEXT CHECK (remote_id IS NULL OR length(remote_id) <= 256),
    checkpoint INTEGER NOT NULL CHECK (checkpoint >= 1),
    PRIMARY KEY (operation_id, chunk)
) STRICT;

-- A fingerprint stays consumed even if the process dies before dropping its transient preview.
CREATE TABLE export_authorizations (
    fingerprint TEXT PRIMARY KEY CHECK (length(fingerprint) = 64),
    operation_id TEXT NOT NULL,
    step_id TEXT NOT NULL,
    FOREIGN KEY (operation_id, step_id) REFERENCES operation_steps (operation_id, step_id),
    UNIQUE (operation_id, step_id)
) STRICT;

CREATE INDEX operations_unfinished_export ON operations (operation_id)
    WHERE kind = 'export' AND state IN ('running', 'paused', 'uncertain');
