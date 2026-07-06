CREATE TABLE antigravity_usage_cache (
    id INTEGER PRIMARY KEY,
    dedupe_key TEXT NOT NULL UNIQUE CHECK (length(trim(dedupe_key)) > 0),
    variant TEXT NOT NULL CHECK (
        variant IN ('antigravity', 'antigravity-ide', 'antigravity-cli')
    ),
    conversation_id TEXT NOT NULL CHECK (length(trim(conversation_id)) > 0),
    response_id TEXT CHECK (response_id IS NULL OR length(trim(response_id)) > 0),
    raw_model_id TEXT NOT NULL CHECK (length(trim(raw_model_id)) > 0),
    model_label TEXT NOT NULL CHECK (length(trim(model_label)) > 0),
    api_provider TEXT,
    input_tokens INTEGER NOT NULL CHECK (input_tokens >= 0),
    output_tokens INTEGER NOT NULL CHECK (output_tokens >= 0),
    thinking_output_tokens INTEGER NOT NULL CHECK (thinking_output_tokens >= 0),
    response_output_tokens INTEGER NOT NULL CHECK (response_output_tokens >= 0),
    cache_read_tokens INTEGER NOT NULL CHECK (cache_read_tokens >= 0),
    cache_write_tokens INTEGER NOT NULL CHECK (cache_write_tokens >= 0),
    observed_at_ms INTEGER NOT NULL CHECK (observed_at_ms >= 0),
    collector_version TEXT NOT NULL CHECK (length(trim(collector_version)) > 0),
    first_seen_at_ms INTEGER NOT NULL CHECK (first_seen_at_ms >= 0),
    last_seen_at_ms INTEGER NOT NULL CHECK (last_seen_at_ms >= first_seen_at_ms)
) STRICT;

CREATE INDEX idx_antigravity_usage_cache_scope
    ON antigravity_usage_cache (variant, conversation_id, observed_at_ms);