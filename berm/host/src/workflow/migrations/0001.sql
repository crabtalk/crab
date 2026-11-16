CREATE TABLE images (
    digest TEXT PRIMARY KEY,
    image BLOB NOT NULL,
    bytes INTEGER NOT NULL CHECK (bytes >= 0)
);
CREATE TABLE runs (
    id TEXT PRIMARY KEY,
    scope TEXT NOT NULL,
    service TEXT NOT NULL,
    workflow TEXT NOT NULL,
    image TEXT NOT NULL REFERENCES images(digest),
    capacity INTEGER NOT NULL CHECK (capacity > 0),
    expires_at INTEGER NOT NULL,
    body BLOB NOT NULL,
    entry BLOB NOT NULL,
    claim TEXT,
    lease INTEGER,
    invalidated INTEGER NOT NULL DEFAULT 0,
    CHECK ((claim IS NULL) = (lease IS NULL))
);
CREATE INDEX run_scope ON runs(scope);
CREATE INDEX run_expiration ON runs(expires_at);
