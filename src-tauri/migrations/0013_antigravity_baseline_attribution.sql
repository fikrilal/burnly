ALTER TABLE antigravity_usage_cache
    ADD COLUMN calendar_attribution TEXT NOT NULL DEFAULT 'dated'
    CHECK (calendar_attribution IN ('dated', 'undated_baseline'));

CREATE INDEX idx_antigravity_usage_cache_calendar_scope
    ON antigravity_usage_cache (variant, calendar_attribution, observed_at_ms);

CREATE TABLE antigravity_baseline_state (
    variant TEXT PRIMARY KEY CHECK (
        variant IN ('antigravity', 'antigravity-ide', 'antigravity-cli')
    ),
    status TEXT NOT NULL CHECK (status IN ('pending', 'complete')),
    started_at_ms INTEGER NOT NULL CHECK (started_at_ms >= 0),
    completed_at_ms INTEGER CHECK (
        (status = 'pending' AND completed_at_ms IS NULL)
        OR (status = 'complete' AND completed_at_ms IS NOT NULL AND completed_at_ms >= started_at_ms)
    ),
    updated_at_ms INTEGER NOT NULL CHECK (updated_at_ms >= started_at_ms)
) STRICT;

CREATE TABLE antigravity_baseline_repair_state (
    repair_version INTEGER PRIMARY KEY CHECK (repair_version > 0),
    stage TEXT NOT NULL CHECK (
        stage IN (
            'not_started',
            'cache_reclassified',
            'canonical_corrected',
            'sync_scheduled',
            'complete',
            'skipped'
        )
    ),
    records_reclassified INTEGER NOT NULL DEFAULT 0 CHECK (records_reclassified >= 0),
    import_run_id INTEGER,
    interval_started_at_ms INTEGER CHECK (
        interval_started_at_ms IS NULL OR interval_started_at_ms >= 0
    ),
    interval_finished_at_ms INTEGER CHECK (
        interval_finished_at_ms IS NULL OR interval_finished_at_ms >= interval_started_at_ms
    ),
    stage_updated_at_ms INTEGER NOT NULL CHECK (stage_updated_at_ms >= 0),
    skip_reason TEXT,
    FOREIGN KEY (import_run_id) REFERENCES import_runs(id) ON DELETE SET NULL
) STRICT;
