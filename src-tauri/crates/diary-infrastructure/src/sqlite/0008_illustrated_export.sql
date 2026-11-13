-- Historical operations and destination hashes are preserved verbatim.
ALTER TABLE export_operations ADD COLUMN content_kind TEXT NOT NULL DEFAULT 'authentic'
    CHECK (content_kind IN ('authentic','illustrated'));
ALTER TABLE export_operations ADD COLUMN illustrated_version TEXT
    CHECK (illustrated_version IS NULL OR length(illustrated_version)=36);
CREATE TABLE export_photos (
    operation_id TEXT NOT NULL REFERENCES export_operations(operation_id),
    n INTEGER NOT NULL CHECK (n BETWEEN 0 AND 98),
    jpeg BLOB NOT NULL CHECK (length(jpeg) BETWEEN 1 AND 5242880),
    PRIMARY KEY(operation_id,n)
) STRICT;
ALTER TABLE notion_steps RENAME TO notion_steps_old;
CREATE TABLE notion_steps (
    destination TEXT NOT NULL CHECK (length(destination)=64),
    date TEXT NOT NULL CHECK (length(date)=10),
    seq INTEGER NOT NULL CHECK (seq>=1),
    kind TEXT NOT NULL CHECK (kind IN ('create_page','create_section','append','delete','create_upload','send_upload','verify')),
    parent TEXT NOT NULL CHECK (length(parent)<=64),
    digest TEXT NOT NULL CHECK (digest='' OR length(digest)=64),
    state TEXT NOT NULL CHECK (state IN ('pending','done','failed')),
    remote_ids TEXT CHECK (remote_ids IS NULL OR length(remote_ids)<=1024),
    PRIMARY KEY(destination,date,seq)
) STRICT;
INSERT INTO notion_steps SELECT * FROM notion_steps_old;
DROP TABLE notion_steps_old;
