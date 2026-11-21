CREATE TABLE pages (
    site TEXT NOT NULL,
    url TEXT NOT NULL,
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
    scanned_at INTEGER,
    scanned_run TEXT,
    PRIMARY KEY (site, url)
);

CREATE TABLE reports (
    site TEXT PRIMARY KEY NOT NULL,
    run TEXT NOT NULL,
    report TEXT NOT NULL,
    unresolved TEXT NOT NULL,
    saved_at INTEGER NOT NULL,
    cursor TEXT,
    receipts TEXT NOT NULL DEFAULT '{}'
) WITHOUT ROWID;

CREATE TABLE runs (
    site TEXT PRIMARY KEY NOT NULL,
    run TEXT NOT NULL,
    expires_at INTEGER NOT NULL,
    robots TEXT,
    sitemap_complete INTEGER NOT NULL DEFAULT 0,
    unlisted TEXT,
    analysis TEXT
) WITHOUT ROWID;

CREATE VIEW unlisted_pages AS
SELECT runs.site, urls.value AS url
FROM runs, json_each(runs.unlisted) batches, json_each(batches.value) urls;
