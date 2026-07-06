CREATE TABLE grok_usage_cache (
    id INTEGER PRIMARY KEY,
    dedupe_key TEXT NOT NULL UNIQUE CHECK (length(trim(dedupe_key)) > 0),
    session_id TEXT NOT NULL CHECK (length(trim(session_id)) > 0),
    observed_at_ms INTEGER NOT NULL CHECK (observed_at_ms >= 0),
    loop_index INTEGER NOT NULL CHECK (loop_index >= 0),
    pid INTEGER NOT NULL CHECK (pid >= 0),
    raw_model_id TEXT NOT NULL CHECK (length(trim(raw_model_id)) > 0),
    model_display_name TEXT CHECK (
        model_display_name IS NULL OR length(trim(model_display_name)) > 0
    ),
    project_path TEXT,
    prompt_tokens INTEGER NOT NULL CHECK (prompt_tokens >= 0),
    cached_prompt_tokens INTEGER NOT NULL CHECK (cached_prompt_tokens >= 0),
    completion_tokens INTEGER NOT NULL CHECK (completion_tokens >= 0),
    reasoning_tokens INTEGER NOT NULL CHECK (reasoning_tokens >= 0),
    log_offset INTEGER NOT NULL CHECK (log_offset >= 0),
    collector_version TEXT NOT NULL CHECK (length(trim(collector_version)) > 0),
    first_seen_at_ms INTEGER NOT NULL CHECK (first_seen_at_ms >= 0),
    last_seen_at_ms INTEGER NOT NULL CHECK (last_seen_at_ms >= first_seen_at_ms),
    CHECK (cached_prompt_tokens <= prompt_tokens)
) STRICT;

CREATE INDEX idx_grok_usage_cache_scope
    ON grok_usage_cache (session_id, observed_at_ms);

CREATE TABLE grok_unified_log_checkpoint (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    file_inode INTEGER CHECK (file_inode IS NULL OR file_inode >= 0),
    file_size INTEGER NOT NULL CHECK (file_size >= 0),
    byte_offset INTEGER NOT NULL CHECK (byte_offset >= 0),
    updated_at_ms INTEGER NOT NULL CHECK (updated_at_ms >= 0)
) STRICT;