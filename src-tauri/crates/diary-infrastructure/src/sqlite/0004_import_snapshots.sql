CREATE TABLE import_snapshots (
    import_id TEXT PRIMARY KEY REFERENCES import_contexts (import_id),
    payload BLOB NOT NULL CHECK (length(payload) BETWEEN 1 AND 16777216)
) STRICT;

CREATE TRIGGER immutable_import_snapshot_update BEFORE UPDATE ON import_snapshots
BEGIN SELECT RAISE(ABORT, 'immutable_import_snapshot'); END;
CREATE TRIGGER immutable_import_snapshot_delete BEFORE DELETE ON import_snapshots
BEGIN SELECT RAISE(ABORT, 'immutable_import_snapshot'); END;
