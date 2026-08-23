ALTER TABLE antigravity_usage_cache
    ADD COLUMN source_record_index INTEGER
    CHECK (source_record_index IS NULL OR source_record_index >= 0);

ALTER TABLE antigravity_usage_cache
    ADD COLUMN timestamp_origin TEXT NOT NULL DEFAULT 'legacy_unknown'
    CHECK (timestamp_origin IN ('source_reported', 'first_seen', 'legacy_unknown'));

CREATE INDEX idx_antigravity_usage_cache_source_record
    ON antigravity_usage_cache (variant, conversation_id, source_record_index);
