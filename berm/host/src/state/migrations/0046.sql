CREATE TABLE caller_files (
    name TEXT PRIMARY KEY,
    id TEXT NOT NULL UNIQUE,
    key BLOB NOT NULL,
    token TEXT NOT NULL,
    bytes INTEGER NOT NULL,
    plan_limit INTEGER NOT NULL,
    writable INTEGER NOT NULL,
    pending INTEGER NOT NULL,
    called_at INTEGER NOT NULL
);
