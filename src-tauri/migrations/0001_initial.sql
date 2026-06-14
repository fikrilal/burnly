CREATE TABLE sources (
    id INTEGER PRIMARY KEY,
    source_key TEXT NOT NULL UNIQUE CHECK (length(trim(source_key)) > 0),
    display_name TEXT NOT NULL CHECK (length(trim(display_name)) > 0),
    enabled INTEGER NOT NULL CHECK (enabled IN (0, 1)),
    detection_state TEXT NOT NULL CHECK (
        detection_state IN (
            'unknown', 'available', 'not_found', 'permission_denied', 'unsupported'
        )
    ),
    first_detected_at_ms INTEGER CHECK (first_detected_at_ms IS NULL OR first_detected_at_ms >= 0),
    last_checked_at_ms INTEGER CHECK (last_checked_at_ms IS NULL OR last_checked_at_ms >= 0),
    last_available_at_ms INTEGER CHECK (last_available_at_ms IS NULL OR last_available_at_ms >= 0),
    created_at_ms INTEGER NOT NULL CHECK (created_at_ms >= 0),
    updated_at_ms INTEGER NOT NULL CHECK (updated_at_ms >= created_at_ms)
) STRICT;

CREATE TABLE source_models (
    id INTEGER PRIMARY KEY,
    source_id INTEGER NOT NULL,
    raw_model_id TEXT NOT NULL CHECK (length(trim(raw_model_id)) > 0),
    display_name TEXT,
    provider_key TEXT,
    first_seen_at_ms INTEGER NOT NULL CHECK (first_seen_at_ms >= 0),
    last_seen_at_ms INTEGER NOT NULL CHECK (last_seen_at_ms >= first_seen_at_ms),
    UNIQUE (source_id, raw_model_id),
    UNIQUE (id, source_id),
    FOREIGN KEY (source_id) REFERENCES sources(id) ON DELETE RESTRICT
) STRICT;

CREATE TABLE projects (
    id INTEGER PRIMARY KEY,
    source_id INTEGER NOT NULL,
    identity_key TEXT NOT NULL CHECK (length(trim(identity_key)) > 0),
    identity_kind TEXT NOT NULL CHECK (identity_kind IN ('path', 'source_key', 'label')),
    raw_path TEXT,
    path_fingerprint BLOB,
    display_name TEXT,
    first_seen_at_ms INTEGER NOT NULL CHECK (first_seen_at_ms >= 0),
    last_seen_at_ms INTEGER NOT NULL CHECK (last_seen_at_ms >= first_seen_at_ms),
    UNIQUE (source_id, identity_key),
    UNIQUE (id, source_id),
    CHECK (identity_kind <> 'path' OR path_fingerprint IS NOT NULL),
    FOREIGN KEY (source_id) REFERENCES sources(id) ON DELETE RESTRICT
) STRICT;

CREATE TABLE refresh_runs (
    id INTEGER PRIMARY KEY,
    job_key TEXT NOT NULL UNIQUE CHECK (length(trim(job_key)) > 0),
    trigger TEXT NOT NULL CHECK (
        trigger IN ('launch', 'manual', 'scheduled', 'file_change', 'resume', 'reconcile')
    ),
    status TEXT NOT NULL CHECK (
        status IN ('queued', 'running', 'cancelling', 'succeeded', 'partial', 'failed', 'cancelled')
    ),
    started_at_ms INTEGER CHECK (started_at_ms IS NULL OR started_at_ms >= 0),
    finished_at_ms INTEGER CHECK (finished_at_ms IS NULL OR finished_at_ms >= 0),
    requested_by_app_version TEXT NOT NULL CHECK (length(trim(requested_by_app_version)) > 0),
    error_code TEXT,
    error_summary TEXT,
    created_at_ms INTEGER NOT NULL CHECK (created_at_ms >= 0),
    CHECK (
        status NOT IN ('succeeded', 'partial', 'failed', 'cancelled')
        OR finished_at_ms IS NOT NULL
    ),
    CHECK (finished_at_ms IS NULL OR started_at_ms IS NULL OR finished_at_ms >= started_at_ms)
) STRICT;

CREATE TABLE import_runs (
    id INTEGER PRIMARY KEY,
    refresh_run_id INTEGER NOT NULL,
    source_id INTEGER NOT NULL,
    collector_key TEXT NOT NULL CHECK (length(trim(collector_key)) > 0),
    collector_version TEXT NOT NULL CHECK (length(trim(collector_version)) > 0),
    profile_version INTEGER NOT NULL CHECK (profile_version > 0),
    projection TEXT NOT NULL CHECK (projection IN ('daily', 'session')),
    scope_kind TEXT NOT NULL CHECK (scope_kind IN ('full', 'incremental')),
    scope_start_date TEXT CHECK (
        scope_start_date IS NULL OR (
            length(scope_start_date) = 10
            AND substr(scope_start_date, 5, 1) = '-'
            AND substr(scope_start_date, 8, 1) = '-'
        )
    ),
    scope_end_date TEXT CHECK (
        scope_end_date IS NULL OR (
            length(scope_end_date) = 10
            AND substr(scope_end_date, 5, 1) = '-'
            AND substr(scope_end_date, 8, 1) = '-'
        )
    ),
    aggregation_timezone TEXT,
    status TEXT NOT NULL CHECK (status IN ('running', 'succeeded', 'partial', 'failed', 'cancelled')),
    records_seen INTEGER NOT NULL CHECK (records_seen >= 0),
    records_rejected INTEGER NOT NULL CHECK (records_rejected >= 0),
    started_at_ms INTEGER NOT NULL CHECK (started_at_ms >= 0),
    finished_at_ms INTEGER CHECK (finished_at_ms IS NULL OR finished_at_ms >= started_at_ms),
    error_code TEXT,
    error_detail TEXT,
    UNIQUE (id, source_id),
    CHECK (projection <> 'daily' OR length(trim(aggregation_timezone)) > 0),
    CHECK (
        scope_kind <> 'incremental'
        OR (scope_start_date IS NOT NULL AND scope_end_date IS NOT NULL)
    ),
    CHECK (scope_start_date IS NULL OR scope_end_date IS NULL OR scope_start_date <= scope_end_date),
    CHECK (status = 'running' OR finished_at_ms IS NOT NULL),
    FOREIGN KEY (refresh_run_id) REFERENCES refresh_runs(id) ON DELETE CASCADE,
    FOREIGN KEY (source_id) REFERENCES sources(id) ON DELETE RESTRICT
) STRICT;

CREATE TABLE daily_usage (
    id INTEGER PRIMARY KEY,
    source_id INTEGER NOT NULL,
    source_key TEXT NOT NULL CHECK (length(trim(source_key)) > 0),
    identity_version INTEGER NOT NULL CHECK (identity_version > 0),
    usage_date TEXT NOT NULL CHECK (
        length(usage_date) = 10
        AND substr(usage_date, 5, 1) = '-'
        AND substr(usage_date, 8, 1) = '-'
    ),
    aggregation_timezone TEXT NOT NULL CHECK (length(trim(aggregation_timezone)) > 0),
    project_id INTEGER,
    input_tokens INTEGER CHECK (input_tokens IS NULL OR input_tokens >= 0),
    output_tokens INTEGER CHECK (output_tokens IS NULL OR output_tokens >= 0),
    cache_creation_tokens INTEGER CHECK (cache_creation_tokens IS NULL OR cache_creation_tokens >= 0),
    cache_read_tokens INTEGER CHECK (cache_read_tokens IS NULL OR cache_read_tokens >= 0),
    total_tokens INTEGER NOT NULL CHECK (total_tokens >= 0),
    unclassified_tokens INTEGER CHECK (unclassified_tokens IS NULL OR unclassified_tokens >= 0),
    cost_amount_micros INTEGER CHECK (cost_amount_micros IS NULL OR cost_amount_micros >= 0),
    cost_currency TEXT CHECK (
        cost_currency IS NULL OR (
            length(cost_currency) = 3
            AND cost_currency = upper(cost_currency)
            AND cost_currency GLOB '[A-Z][A-Z][A-Z]'
        )
    ),
    cost_kind TEXT NOT NULL CHECK (
        cost_kind IN (
            'source_reported', 'collector_calculated', 'collector_mixed',
            'burnly_calculated', 'unknown'
        )
    ),
    cost_status TEXT NOT NULL CHECK (
        cost_status IN ('available', 'estimated', 'not_applicable', 'unavailable')
    ),
    data_quality TEXT NOT NULL CHECK (length(trim(data_quality)) > 0),
    record_state TEXT NOT NULL CHECK (record_state IN ('active', 'missing', 'removed')),
    absence_count INTEGER NOT NULL CHECK (absence_count >= 0),
    first_seen_at_ms INTEGER NOT NULL CHECK (first_seen_at_ms >= 0),
    last_seen_at_ms INTEGER NOT NULL CHECK (last_seen_at_ms >= first_seen_at_ms),
    removed_at_ms INTEGER CHECK (removed_at_ms IS NULL OR removed_at_ms >= last_seen_at_ms),
    latest_import_id INTEGER NOT NULL,
    UNIQUE (source_id, source_key),
    UNIQUE (id, source_id),
    CHECK (
        (cost_status IN ('available', 'estimated')
            AND cost_amount_micros IS NOT NULL
            AND cost_currency IS NOT NULL)
        OR (cost_status IN ('not_applicable', 'unavailable')
            AND cost_amount_micros IS NULL
            AND cost_currency IS NULL)
    ),
    CHECK (
        (record_state = 'active' AND absence_count = 0 AND removed_at_ms IS NULL)
        OR (record_state = 'missing' AND absence_count = 1 AND removed_at_ms IS NULL)
        OR (record_state = 'removed' AND absence_count >= 2 AND removed_at_ms IS NOT NULL)
    ),
    FOREIGN KEY (source_id) REFERENCES sources(id) ON DELETE RESTRICT,
    FOREIGN KEY (project_id, source_id) REFERENCES projects(id, source_id) ON DELETE RESTRICT,
    FOREIGN KEY (latest_import_id, source_id) REFERENCES import_runs(id, source_id) ON DELETE RESTRICT
) STRICT;

CREATE TABLE daily_model_usage (
    id INTEGER PRIMARY KEY,
    daily_usage_id INTEGER NOT NULL,
    source_id INTEGER NOT NULL,
    model_id INTEGER,
    input_tokens INTEGER CHECK (input_tokens IS NULL OR input_tokens >= 0),
    output_tokens INTEGER CHECK (output_tokens IS NULL OR output_tokens >= 0),
    cache_creation_tokens INTEGER CHECK (cache_creation_tokens IS NULL OR cache_creation_tokens >= 0),
    cache_read_tokens INTEGER CHECK (cache_read_tokens IS NULL OR cache_read_tokens >= 0),
    total_tokens INTEGER CHECK (total_tokens IS NULL OR total_tokens >= 0),
    unclassified_tokens INTEGER CHECK (unclassified_tokens IS NULL OR unclassified_tokens >= 0),
    cost_amount_micros INTEGER CHECK (cost_amount_micros IS NULL OR cost_amount_micros >= 0),
    cost_currency TEXT CHECK (
        cost_currency IS NULL OR (
            length(cost_currency) = 3
            AND cost_currency = upper(cost_currency)
            AND cost_currency GLOB '[A-Z][A-Z][A-Z]'
        )
    ),
    cost_status TEXT NOT NULL CHECK (cost_status IN ('estimated', 'unavailable')),
    latest_import_id INTEGER NOT NULL,
    UNIQUE (daily_usage_id, model_id),
    CHECK (
        (cost_status = 'estimated' AND cost_amount_micros IS NOT NULL AND cost_currency IS NOT NULL)
        OR (cost_status = 'unavailable' AND cost_amount_micros IS NULL AND cost_currency IS NULL)
    ),
    FOREIGN KEY (daily_usage_id, source_id) REFERENCES daily_usage(id, source_id) ON DELETE CASCADE,
    FOREIGN KEY (model_id, source_id) REFERENCES source_models(id, source_id) ON DELETE RESTRICT,
    FOREIGN KEY (latest_import_id, source_id) REFERENCES import_runs(id, source_id) ON DELETE RESTRICT
) STRICT;

CREATE TABLE sessions (
    id INTEGER PRIMARY KEY,
    source_id INTEGER NOT NULL,
    source_key TEXT NOT NULL CHECK (length(trim(source_key)) > 0),
    identity_version INTEGER NOT NULL CHECK (identity_version > 0),
    source_session_id TEXT NOT NULL CHECK (length(trim(source_session_id)) > 0),
    project_id INTEGER,
    first_activity_at_ms INTEGER CHECK (first_activity_at_ms IS NULL OR first_activity_at_ms >= 0),
    last_activity_at_ms INTEGER CHECK (last_activity_at_ms IS NULL OR last_activity_at_ms >= 0),
    input_tokens INTEGER CHECK (input_tokens IS NULL OR input_tokens >= 0),
    output_tokens INTEGER CHECK (output_tokens IS NULL OR output_tokens >= 0),
    cache_creation_tokens INTEGER CHECK (cache_creation_tokens IS NULL OR cache_creation_tokens >= 0),
    cache_read_tokens INTEGER CHECK (cache_read_tokens IS NULL OR cache_read_tokens >= 0),
    total_tokens INTEGER NOT NULL CHECK (total_tokens >= 0),
    unclassified_tokens INTEGER CHECK (unclassified_tokens IS NULL OR unclassified_tokens >= 0),
    cost_amount_micros INTEGER CHECK (cost_amount_micros IS NULL OR cost_amount_micros >= 0),
    cost_currency TEXT CHECK (
        cost_currency IS NULL OR (
            length(cost_currency) = 3
            AND cost_currency = upper(cost_currency)
            AND cost_currency GLOB '[A-Z][A-Z][A-Z]'
        )
    ),
    cost_kind TEXT NOT NULL CHECK (
        cost_kind IN (
            'source_reported', 'collector_calculated', 'collector_mixed',
            'burnly_calculated', 'unknown'
        )
    ),
    cost_status TEXT NOT NULL CHECK (
        cost_status IN ('available', 'estimated', 'not_applicable', 'unavailable')
    ),
    data_quality TEXT NOT NULL CHECK (length(trim(data_quality)) > 0),
    record_state TEXT NOT NULL CHECK (record_state IN ('active', 'missing', 'removed')),
    absence_count INTEGER NOT NULL CHECK (absence_count >= 0),
    first_seen_at_ms INTEGER NOT NULL CHECK (first_seen_at_ms >= 0),
    last_seen_at_ms INTEGER NOT NULL CHECK (last_seen_at_ms >= first_seen_at_ms),
    removed_at_ms INTEGER CHECK (removed_at_ms IS NULL OR removed_at_ms >= last_seen_at_ms),
    latest_import_id INTEGER NOT NULL,
    UNIQUE (source_id, source_key),
    UNIQUE (source_id, source_session_id),
    UNIQUE (id, source_id),
    CHECK (first_activity_at_ms IS NULL OR last_activity_at_ms IS NULL OR first_activity_at_ms <= last_activity_at_ms),
    CHECK (
        (cost_status IN ('available', 'estimated')
            AND cost_amount_micros IS NOT NULL
            AND cost_currency IS NOT NULL)
        OR (cost_status IN ('not_applicable', 'unavailable')
            AND cost_amount_micros IS NULL
            AND cost_currency IS NULL)
    ),
    CHECK (
        (record_state = 'active' AND absence_count = 0 AND removed_at_ms IS NULL)
        OR (record_state = 'missing' AND absence_count = 1 AND removed_at_ms IS NULL)
        OR (record_state = 'removed' AND absence_count >= 2 AND removed_at_ms IS NOT NULL)
    ),
    FOREIGN KEY (source_id) REFERENCES sources(id) ON DELETE RESTRICT,
    FOREIGN KEY (project_id, source_id) REFERENCES projects(id, source_id) ON DELETE RESTRICT,
    FOREIGN KEY (latest_import_id, source_id) REFERENCES import_runs(id, source_id) ON DELETE RESTRICT
) STRICT;

CREATE TABLE session_model_usage (
    id INTEGER PRIMARY KEY,
    session_id INTEGER NOT NULL,
    source_id INTEGER NOT NULL,
    model_id INTEGER,
    input_tokens INTEGER CHECK (input_tokens IS NULL OR input_tokens >= 0),
    output_tokens INTEGER CHECK (output_tokens IS NULL OR output_tokens >= 0),
    cache_creation_tokens INTEGER CHECK (cache_creation_tokens IS NULL OR cache_creation_tokens >= 0),
    cache_read_tokens INTEGER CHECK (cache_read_tokens IS NULL OR cache_read_tokens >= 0),
    total_tokens INTEGER CHECK (total_tokens IS NULL OR total_tokens >= 0),
    unclassified_tokens INTEGER CHECK (unclassified_tokens IS NULL OR unclassified_tokens >= 0),
    cost_amount_micros INTEGER CHECK (cost_amount_micros IS NULL OR cost_amount_micros >= 0),
    cost_currency TEXT CHECK (
        cost_currency IS NULL OR (
            length(cost_currency) = 3
            AND cost_currency = upper(cost_currency)
            AND cost_currency GLOB '[A-Z][A-Z][A-Z]'
        )
    ),
    cost_status TEXT NOT NULL CHECK (cost_status IN ('estimated', 'unavailable')),
    latest_import_id INTEGER NOT NULL,
    UNIQUE (session_id, model_id),
    CHECK (
        (cost_status = 'estimated' AND cost_amount_micros IS NOT NULL AND cost_currency IS NOT NULL)
        OR (cost_status = 'unavailable' AND cost_amount_micros IS NULL AND cost_currency IS NULL)
    ),
    FOREIGN KEY (session_id, source_id) REFERENCES sessions(id, source_id) ON DELETE CASCADE,
    FOREIGN KEY (model_id, source_id) REFERENCES source_models(id, source_id) ON DELETE RESTRICT,
    FOREIGN KEY (latest_import_id, source_id) REFERENCES import_runs(id, source_id) ON DELETE RESTRICT
) STRICT;

CREATE TABLE app_settings (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    reporting_timezone TEXT NOT NULL CHECK (length(trim(reporting_timezone)) > 0),
    background_refresh_enabled INTEGER NOT NULL CHECK (background_refresh_enabled IN (0, 1)),
    refresh_interval_minutes INTEGER NOT NULL CHECK (refresh_interval_minutes > 0),
    launch_at_login INTEGER NOT NULL CHECK (launch_at_login IN (0, 1)),
    close_behavior TEXT NOT NULL CHECK (close_behavior IN ('hide', 'quit')),
    notifications_enabled INTEGER NOT NULL CHECK (notifications_enabled IN (0, 1)),
    store_project_paths INTEGER NOT NULL CHECK (store_project_paths IN (0, 1)),
    created_at_ms INTEGER NOT NULL CHECK (created_at_ms >= 0),
    updated_at_ms INTEGER NOT NULL CHECK (updated_at_ms >= created_at_ms)
) STRICT;

CREATE TABLE budgets (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL CHECK (length(trim(name)) > 0),
    metric TEXT NOT NULL CHECK (metric IN ('tokens', 'cost')),
    period TEXT NOT NULL CHECK (period IN ('daily', 'weekly', 'monthly')),
    limit_value INTEGER NOT NULL CHECK (limit_value > 0),
    currency TEXT CHECK (
        currency IS NULL OR (
            length(currency) = 3
            AND currency = upper(currency)
            AND currency GLOB '[A-Z][A-Z][A-Z]'
        )
    ),
    source_id INTEGER,
    enabled INTEGER NOT NULL CHECK (enabled IN (0, 1)),
    created_at_ms INTEGER NOT NULL CHECK (created_at_ms >= 0),
    updated_at_ms INTEGER NOT NULL CHECK (updated_at_ms >= created_at_ms),
    CHECK (
        (metric = 'cost' AND currency IS NOT NULL)
        OR (metric = 'tokens' AND currency IS NULL)
    ),
    FOREIGN KEY (source_id) REFERENCES sources(id) ON DELETE RESTRICT
) STRICT;

CREATE TABLE budget_thresholds (
    budget_id INTEGER NOT NULL,
    threshold_bps INTEGER NOT NULL CHECK (threshold_bps > 0),
    enabled INTEGER NOT NULL CHECK (enabled IN (0, 1)),
    PRIMARY KEY (budget_id, threshold_bps),
    FOREIGN KEY (budget_id) REFERENCES budgets(id) ON DELETE CASCADE
) STRICT;

CREATE TABLE budget_notification_state (
    budget_id INTEGER NOT NULL,
    period_start_date TEXT NOT NULL CHECK (
        length(period_start_date) = 10
        AND substr(period_start_date, 5, 1) = '-'
        AND substr(period_start_date, 8, 1) = '-'
    ),
    aggregation_timezone TEXT NOT NULL CHECK (length(trim(aggregation_timezone)) > 0),
    threshold_bps INTEGER NOT NULL,
    observed_value INTEGER NOT NULL CHECK (observed_value >= 0),
    notified_at_ms INTEGER NOT NULL CHECK (notified_at_ms >= 0),
    delivery_status TEXT NOT NULL CHECK (delivery_status IN ('delivered', 'failed', 'suppressed')),
    PRIMARY KEY (budget_id, period_start_date, aggregation_timezone, threshold_bps),
    FOREIGN KEY (budget_id, threshold_bps)
        REFERENCES budget_thresholds(budget_id, threshold_bps) ON DELETE CASCADE
) STRICT;

CREATE UNIQUE INDEX daily_model_usage_unknown_model
    ON daily_model_usage(daily_usage_id)
    WHERE model_id IS NULL;
CREATE UNIQUE INDEX session_model_usage_unknown_model
    ON session_model_usage(session_id)
    WHERE model_id IS NULL;

CREATE INDEX daily_usage_by_date ON daily_usage(usage_date);
CREATE INDEX daily_usage_by_source_date ON daily_usage(source_id, usage_date);
CREATE INDEX daily_usage_by_project_date ON daily_usage(project_id, usage_date)
    WHERE project_id IS NOT NULL;
CREATE INDEX daily_usage_by_import ON daily_usage(latest_import_id);
CREATE INDEX daily_usage_active_by_date_source ON daily_usage(usage_date, source_id)
    WHERE record_state <> 'removed';

CREATE INDEX sessions_by_activity ON sessions(last_activity_at_ms DESC, id DESC);
CREATE INDEX sessions_by_source_activity ON sessions(source_id, last_activity_at_ms DESC, id DESC);
CREATE INDEX sessions_by_project_activity ON sessions(project_id, last_activity_at_ms DESC, id DESC)
    WHERE project_id IS NOT NULL;
CREATE INDEX sessions_by_import ON sessions(latest_import_id);

CREATE INDEX daily_model_usage_by_model ON daily_model_usage(model_id, daily_usage_id);
CREATE INDEX session_model_usage_by_model ON session_model_usage(model_id, session_id);
CREATE INDEX daily_model_usage_by_parent ON daily_model_usage(daily_usage_id);
CREATE INDEX session_model_usage_by_parent ON session_model_usage(session_id);

CREATE INDEX refresh_runs_by_created ON refresh_runs(created_at_ms DESC);
CREATE INDEX import_runs_by_refresh ON import_runs(refresh_run_id);
CREATE INDEX import_runs_by_source_projection_started
    ON import_runs(source_id, projection, started_at_ms DESC);

CREATE INDEX budgets_by_enabled_metric_period ON budgets(enabled, metric, period);
CREATE INDEX budget_notifications_by_budget_period
    ON budget_notification_state(budget_id, period_start_date);
