CREATE TABLE opencode_session_checkpoint (
    session_id TEXT PRIMARY KEY CHECK (length(trim(session_id)) > 0),
    accepted_input_tokens INTEGER NOT NULL CHECK (accepted_input_tokens >= 0),
    accepted_output_tokens INTEGER NOT NULL CHECK (accepted_output_tokens >= 0),
    accepted_reasoning_tokens INTEGER NOT NULL CHECK (accepted_reasoning_tokens >= 0),
    accepted_cache_read_tokens INTEGER NOT NULL CHECK (accepted_cache_read_tokens >= 0),
    accepted_cache_write_tokens INTEGER NOT NULL CHECK (accepted_cache_write_tokens >= 0),
    accepted_cost_micros INTEGER CHECK (accepted_cost_micros IS NULL OR accepted_cost_micros >= 0),
    observed_input_tokens INTEGER NOT NULL CHECK (observed_input_tokens >= 0),
    observed_output_tokens INTEGER NOT NULL CHECK (observed_output_tokens >= 0),
    observed_reasoning_tokens INTEGER NOT NULL CHECK (observed_reasoning_tokens >= 0),
    observed_cache_read_tokens INTEGER NOT NULL CHECK (observed_cache_read_tokens >= 0),
    observed_cache_write_tokens INTEGER NOT NULL CHECK (observed_cache_write_tokens >= 0),
    observed_cost_micros INTEGER CHECK (observed_cost_micros IS NULL OR observed_cost_micros >= 0),
    source_updated_at_ms INTEGER NOT NULL CHECK (source_updated_at_ms >= 0),
    reconciliation_state TEXT NOT NULL CHECK (
        reconciliation_state IN ('complete', 'partial', 'deferred_live_write')
    ),
    next_recovery_sequence INTEGER NOT NULL CHECK (next_recovery_sequence >= 0),
    first_observed_at_ms INTEGER NOT NULL CHECK (first_observed_at_ms >= 0),
    last_reconciled_at_ms INTEGER NOT NULL CHECK (last_reconciled_at_ms >= first_observed_at_ms)
) STRICT;

CREATE TABLE opencode_usage_ledger (
    id INTEGER PRIMARY KEY,
    source_message_id TEXT UNIQUE CHECK (
        source_message_id IS NULL OR length(trim(source_message_id)) > 0
    ),
    recovery_sequence INTEGER CHECK (recovery_sequence IS NULL OR recovery_sequence >= 0),
    session_id TEXT NOT NULL REFERENCES opencode_session_checkpoint(session_id) ON DELETE CASCADE,
    activity_at_ms INTEGER NOT NULL CHECK (activity_at_ms >= 0),
    timestamp_origin TEXT NOT NULL CHECK (
        timestamp_origin IN ('source_reported', 'source_lifecycle', 'first_seen')
    ),
    provider_id TEXT CHECK (provider_id IS NULL OR length(trim(provider_id)) > 0),
    raw_model_id TEXT NOT NULL CHECK (length(trim(raw_model_id)) > 0),
    input_tokens INTEGER NOT NULL CHECK (input_tokens >= 0),
    output_tokens INTEGER NOT NULL CHECK (output_tokens >= 0),
    reasoning_tokens INTEGER NOT NULL CHECK (reasoning_tokens >= 0),
    cache_read_tokens INTEGER NOT NULL CHECK (cache_read_tokens >= 0),
    cache_write_tokens INTEGER NOT NULL CHECK (cache_write_tokens >= 0),
    cost_micros INTEGER CHECK (cost_micros IS NULL OR cost_micros >= 0),
    origin TEXT NOT NULL CHECK (
        origin IN ('v1_message', 'v2_message', 'cumulative_recovery')
    ),
    data_quality TEXT NOT NULL CHECK (data_quality IN ('complete', 'partial')),
    first_seen_at_ms INTEGER NOT NULL CHECK (first_seen_at_ms >= 0),
    last_seen_at_ms INTEGER NOT NULL CHECK (last_seen_at_ms >= first_seen_at_ms),
    UNIQUE (session_id, recovery_sequence),
    CHECK (
        (
            origin IN ('v1_message', 'v2_message')
            AND source_message_id IS NOT NULL
            AND recovery_sequence IS NULL
            AND timestamp_origin = 'source_reported'
            AND provider_id IS NOT NULL
            AND data_quality = 'complete'
        ) OR (
            origin = 'cumulative_recovery'
            AND source_message_id IS NULL
            AND recovery_sequence IS NOT NULL
            AND timestamp_origin IN ('source_lifecycle', 'first_seen')
            AND provider_id IS NULL
            AND raw_model_id = 'OpenCode unattributed'
            AND data_quality = 'partial'
        )
    )
) STRICT;

CREATE INDEX idx_opencode_usage_ledger_session_activity
    ON opencode_usage_ledger (session_id, activity_at_ms, id);
