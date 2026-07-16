-- Dedicated durable state for desktop collect sync (upload outbox).
-- Does not alter app_settings or usage fact tables.

CREATE TABLE collect_sync_state (
    user_id TEXT NOT NULL CHECK (length(trim(user_id)) > 0),
    client_device_id TEXT NOT NULL CHECK (length(trim(client_device_id)) > 0),
    next_client_revision INTEGER NOT NULL DEFAULT 1
        CHECK (next_client_revision > 0),
    baseline_status TEXT NOT NULL DEFAULT 'none'
        CHECK (baseline_status IN ('none', 'in_progress', 'complete')),
    pending_scope_json TEXT,
    active_generation_id TEXT,
    last_attempt_at_ms INTEGER CHECK (
        last_attempt_at_ms IS NULL OR last_attempt_at_ms >= 0
    ),
    last_accepted_at_ms INTEGER CHECK (
        last_accepted_at_ms IS NULL OR last_accepted_at_ms >= 0
    ),
    last_error_code TEXT,
    last_error_message TEXT,
    last_error_retryable INTEGER CHECK (
        last_error_retryable IS NULL OR last_error_retryable IN (0, 1)
    ),
    device_metadata_fingerprint TEXT,
    device_registered_revision INTEGER CHECK (
        device_registered_revision IS NULL OR device_registered_revision > 0
    ),
    created_at_ms INTEGER NOT NULL CHECK (created_at_ms >= 0),
    updated_at_ms INTEGER NOT NULL CHECK (updated_at_ms >= created_at_ms),
    PRIMARY KEY (user_id, client_device_id)
) STRICT;

CREATE TABLE collect_sync_outbox (
    id INTEGER PRIMARY KEY,
    user_id TEXT NOT NULL CHECK (length(trim(user_id)) > 0),
    client_device_id TEXT NOT NULL CHECK (length(trim(client_device_id)) > 0),
    generation_id TEXT NOT NULL CHECK (length(trim(generation_id)) > 0),
    batch_index INTEGER NOT NULL CHECK (batch_index >= 0),
    batch_count INTEGER NOT NULL CHECK (batch_count > 0),
    client_revision INTEGER NOT NULL CHECK (client_revision > 0),
    idempotency_key TEXT NOT NULL CHECK (length(trim(idempotency_key)) > 0),
    request_body TEXT NOT NULL CHECK (length(request_body) > 0),
    payload_hash TEXT NOT NULL CHECK (length(trim(payload_hash)) > 0),
    window_scope TEXT NOT NULL CHECK (window_scope IN ('full', 'incremental')),
    window_start TEXT NOT NULL CHECK (
        length(window_start) = 10
        AND substr(window_start, 5, 1) = '-'
        AND substr(window_start, 8, 1) = '-'
    ),
    window_end TEXT NOT NULL CHECK (
        length(window_end) = 10
        AND substr(window_end, 5, 1) = '-'
        AND substr(window_end, 8, 1) = '-'
    ),
    status TEXT NOT NULL CHECK (status IN ('pending', 'accepted')),
    created_at_ms INTEGER NOT NULL CHECK (created_at_ms >= 0),
    accepted_at_ms INTEGER CHECK (
        accepted_at_ms IS NULL OR accepted_at_ms >= created_at_ms
    ),
    UNIQUE (user_id, client_device_id, generation_id, batch_index),
    UNIQUE (user_id, client_device_id, client_revision),
    UNIQUE (idempotency_key),
    CHECK (
        (status = 'pending' AND accepted_at_ms IS NULL)
        OR (status = 'accepted' AND accepted_at_ms IS NOT NULL)
    ),
    CHECK (batch_index < batch_count),
    CHECK (window_end >= window_start)
) STRICT;

CREATE INDEX collect_sync_outbox_pending_order
ON collect_sync_outbox (
    user_id,
    client_device_id,
    status,
    client_revision
);
