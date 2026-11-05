CREATE TABLE import_contexts (
    import_id TEXT PRIMARY KEY CHECK (length(import_id) BETWEEN 1 AND 64),
    diary_date TEXT NOT NULL CHECK (length(diary_date) = 10),
    zone TEXT NOT NULL CHECK (length(zone) BETWEEN 1 AND 128),
    database_version TEXT NOT NULL CHECK (length(database_version) BETWEEN 1 AND 32),
    config_generation INTEGER NOT NULL CHECK (config_generation >= 0),
    completed_at INTEGER
) STRICT;

CREATE TABLE source_identities (
    source_id TEXT PRIMARY KEY CHECK (length(source_id) BETWEEN 1 AND 128),
    namespace BLOB NOT NULL CHECK (length(namespace) = 32),
    path_key TEXT NOT NULL CHECK (length(path_key) BETWEEN 1 AND 4096),
    UNIQUE (namespace, path_key)
) STRICT;

CREATE TABLE time_resolutions (
    source_key TEXT NOT NULL REFERENCES source_identities (source_id),
    digest BLOB NOT NULL CHECK (length(digest) = 32),
    authority TEXT NOT NULL CHECK (authority IN ('header', 'filename')),
    instant_ms INTEGER NOT NULL,
    precision TEXT NOT NULL CHECK (precision IN ('second', 'milli')),
    decided_at INTEGER NOT NULL,
    PRIMARY KEY (source_key, digest)
) STRICT;

-- Provenance is immutable; completion is a one-way marker.
CREATE TRIGGER immutable_import_context BEFORE UPDATE ON import_contexts
WHEN NEW.import_id != OLD.import_id OR NEW.diary_date != OLD.diary_date
    OR NEW.zone != OLD.zone OR NEW.database_version != OLD.database_version
    OR NEW.config_generation != OLD.config_generation OR OLD.completed_at IS NOT NULL
BEGIN SELECT RAISE(ABORT, 'immutable_import_context'); END;
