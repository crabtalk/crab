-- Each watched domain: its robots.txt as last read, its Content-Signal, when it was last read, and the reads that failed since.

CREATE TABLE crawl_sites (
    domain TEXT PRIMARY KEY,
    robots TEXT,
    signal TEXT,
    read INTEGER NOT NULL DEFAULT 0,
    failures INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX crawl_sites_read ON crawl_sites (read);
