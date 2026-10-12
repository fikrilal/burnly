CREATE TABLE antigravity_conversation_checkpoint (
    fingerprint TEXT PRIMARY KEY CHECK (length(fingerprint) = 64),
    file_length INTEGER NOT NULL CHECK (file_length >= 0),
    file_mtime_ms INTEGER NOT NULL,
    variant TEXT NOT NULL CHECK (
        variant IN ('antigravity', 'antigravity-ide', 'antigravity-cli')
    ),
    conversation_id TEXT NOT NULL CHECK (length(trim(conversation_id)) > 0)
) STRICT;
