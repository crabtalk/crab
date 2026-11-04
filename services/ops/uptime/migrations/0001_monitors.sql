CREATE TABLE uptime_monitors (
    name TEXT PRIMARY KEY,
    url TEXT NOT NULL,
    interval_ms INTEGER NOT NULL,
    expected_status INTEGER,
    due_at INTEGER NOT NULL,
    checked_at INTEGER,
    state TEXT NOT NULL DEFAULT 'unknown',
    failures INTEGER NOT NULL DEFAULT 0,
    status INTEGER,
    latency_ms INTEGER,
    error TEXT
);
CREATE INDEX uptime_due ON uptime_monitors (due_at, name);
CREATE TABLE uptime_checks (
    seq INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    checked_at INTEGER NOT NULL,
    ok INTEGER NOT NULL,
    status INTEGER,
    latency_ms INTEGER NOT NULL,
    error TEXT
);
CREATE INDEX uptime_history ON uptime_checks (name, seq);
