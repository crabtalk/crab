-- This deployment's sites, every view recorded for one in the last 30 days, and each day's views by page, source and crawler.

CREATE TABLE sites (
    id TEXT PRIMARY KEY,
    domain TEXT NOT NULL,
    name TEXT NOT NULL,
    created INTEGER NOT NULL,
    UNIQUE (domain)
);

CREATE TABLE events (
    seq INTEGER PRIMARY KEY,
    site TEXT NOT NULL,
    at INTEGER NOT NULL,
    page TEXT NOT NULL,
    source TEXT NOT NULL,
    crawler TEXT NOT NULL
);

CREATE INDEX events_site ON events (site, seq);
CREATE INDEX events_at ON events (at);

CREATE TABLE days (
    site TEXT NOT NULL,
    day INTEGER NOT NULL,
    page TEXT NOT NULL,
    source TEXT NOT NULL,
    crawler TEXT NOT NULL,
    views INTEGER NOT NULL,
    PRIMARY KEY (site, day, page, source, crawler)
);
