ALTER TABLE opencode_session_checkpoint
    ADD COLUMN last_source_message_id TEXT;

CREATE TABLE opencode_redundancy_proof (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    file_length INTEGER NOT NULL CHECK (file_length >= 0),
    file_mtime_ms INTEGER NOT NULL,
    page_count INTEGER NOT NULL CHECK (page_count >= 0),
    ignored_max_rowid INTEGER,
    ignored_row_count INTEGER NOT NULL CHECK (ignored_row_count >= 0),
    generations TEXT NOT NULL CHECK (generations IN ('v1-ignored', 'v2-ignored')),
    proved_redundant INTEGER NOT NULL CHECK (proved_redundant IN (0, 1))
) STRICT;
