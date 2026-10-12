CREATE TABLE opencode_deferred_message (
    session_id TEXT NOT NULL,
    message_id TEXT NOT NULL CHECK (length(trim(message_id)) > 0),
    PRIMARY KEY (session_id, message_id),
    FOREIGN KEY (session_id) REFERENCES opencode_session_checkpoint (session_id) ON DELETE CASCADE
) STRICT;
