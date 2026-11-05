-- Rebuildable from the Authentic Markdown files; never an authority for receipts.
CREATE TABLE catalog_authentic (
    date TEXT PRIMARY KEY CHECK (length(date) = 10),
    id TEXT,
    revision INTEGER,
    sha256 TEXT,
    timezone TEXT,
    size INTEGER NOT NULL,
    mtime_ns INTEGER NOT NULL,
    inode TEXT NOT NULL,
    -- NULL when the file parses; otherwise the readable diagnostic that blocks saving.
    problem TEXT CHECK (problem IS NULL OR length(problem) <= 256)
) STRICT;
CREATE UNIQUE INDEX catalog_authentic_id ON catalog_authentic (id) WHERE id IS NOT NULL;
