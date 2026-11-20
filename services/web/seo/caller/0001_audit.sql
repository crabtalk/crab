CREATE TABLE pages (
    site TEXT NOT NULL,
    url TEXT NOT NULL,
    listed_run TEXT NOT NULL,
    blocked INTEGER NOT NULL,
    etag TEXT,
    last_modified TEXT,
    status INTEGER,
    problem TEXT,
    redirect TEXT,
    hash TEXT,
    checked_fields TEXT,
    scan_hash TEXT,
    scan TEXT,
    checked_at INTEGER,
    scanned_at INTEGER,
    checked_run TEXT,
    scanned_run TEXT,
    PRIMARY KEY (site, url)
);

CREATE INDEX pages_check_order ON pages (site, checked_at, url);

CREATE TABLE reports (
    site TEXT PRIMARY KEY NOT NULL,
    run TEXT NOT NULL,
    report TEXT NOT NULL,
    unresolved TEXT NOT NULL,
    saved_at INTEGER NOT NULL,
    receipts TEXT NOT NULL DEFAULT '{}'
) WITHOUT ROWID;

CREATE TABLE runs (
    site TEXT PRIMARY KEY NOT NULL,
    run TEXT NOT NULL,
    expires_at INTEGER NOT NULL,
    robots TEXT,
    sitemap_complete INTEGER NOT NULL DEFAULT 0,
    analysis TEXT
) WITHOUT ROWID;
