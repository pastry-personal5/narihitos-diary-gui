-- Adapter-internal progress of a multi-request Notion write. Rows are content-free: the
-- destination is a hash, `parent` and `remote_ids` are Notion object IDs, `digest` is a chunk
-- digest. They explain a partial write after a crash; recovery reads the remote back and never
-- replays a step. Rows are cleared when the next write for the same date begins.
CREATE TABLE notion_steps (
    destination TEXT NOT NULL CHECK (length(destination) = 64),
    date TEXT NOT NULL CHECK (length(date) = 10),
    seq INTEGER NOT NULL CHECK (seq >= 1),
    kind TEXT NOT NULL CHECK (kind IN ('create_page', 'create_section', 'append', 'delete')),
    parent TEXT NOT NULL CHECK (length(parent) <= 64),
    digest TEXT NOT NULL CHECK (digest = '' OR length(digest) = 64),
    state TEXT NOT NULL CHECK (state IN ('pending', 'done', 'failed')),
    remote_ids TEXT CHECK (remote_ids IS NULL OR length(remote_ids) <= 1024),
    PRIMARY KEY (destination, date, seq)
) STRICT;
