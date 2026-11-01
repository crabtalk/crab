CREATE TABLE release_watch_watches (
    name TEXT PRIMARY KEY,
    spec TEXT NOT NULL,
    attempted_at INTEGER,
    succeeded_at INTEGER,
    error TEXT,
    snapshot TEXT
);
CREATE TABLE release_watch_changes (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    observed_at INTEGER NOT NULL,
    spec TEXT NOT NULL,
    kind TEXT NOT NULL,
    snapshot TEXT,
    error TEXT
);
