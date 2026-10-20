CREATE TABLE metric_counters (name TEXT PRIMARY KEY, count INTEGER NOT NULL DEFAULT 0);
CREATE TABLE metric_durations (
    kind TEXT NOT NULL,
    duration_ms INTEGER NOT NULL,
    count INTEGER NOT NULL,
    PRIMARY KEY (kind, duration_ms)
);
