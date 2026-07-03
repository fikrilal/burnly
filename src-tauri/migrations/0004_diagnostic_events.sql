CREATE TABLE diagnostic_events (
    id INTEGER PRIMARY KEY,
    area TEXT NOT NULL CHECK (length(trim(area)) > 0),
    severity TEXT NOT NULL CHECK (severity IN ('info', 'warning', 'error')),
    code TEXT NOT NULL CHECK (length(trim(code)) > 0),
    summary TEXT NOT NULL CHECK (length(trim(summary)) > 0),
    context_json TEXT,
    created_at_ms INTEGER NOT NULL CHECK (created_at_ms >= 0)
) STRICT;

CREATE INDEX diagnostic_events_by_created
    ON diagnostic_events(created_at_ms DESC, id DESC);

CREATE INDEX diagnostic_events_by_area_created
    ON diagnostic_events(area, created_at_ms DESC, id DESC);
