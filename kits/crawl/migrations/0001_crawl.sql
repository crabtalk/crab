-- Each watched domain: its robots.txt as last read, its Content-Signal, when it was last read, the reads that failed since, whether it is read before the other sites due, and its last error and when it failed.

CREATE TABLE crawl_sites (
    domain TEXT PRIMARY KEY,
    robots TEXT,
    signal TEXT,
    read INTEGER NOT NULL DEFAULT 0,
    failures INTEGER NOT NULL DEFAULT 0,
    first INTEGER NOT NULL DEFAULT 0,
    error TEXT,
    failed INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX crawl_sites_first ON crawl_sites (first, read);
CREATE INDEX crawl_sites_failed ON crawl_sites (failed);

-- Each URL a site's sitemaps list, or its program or a crawler's links listed, that is of a kind its program names: the sitemap's lastmod as written, whether it was listed, and the most bytes of HTML it is read for, absent for the schedule's page bytes.

CREATE TABLE crawl_urls (
    url TEXT PRIMARY KEY,
    domain TEXT NOT NULL,
    kind TEXT NOT NULL,
    lastmod TEXT,
    listed INTEGER NOT NULL DEFAULT 0,
    bytes INTEGER
);

CREATE INDEX crawl_urls_domain ON crawl_urls (domain, kind);

-- Each page fetched: its status, the validators it answered with, the lastmod its URL had when fetched, its title, description and text, and its last error.

CREATE TABLE crawl_pages (
    url TEXT PRIMARY KEY,
    status INTEGER NOT NULL,
    etag TEXT,
    modified TEXT,
    lastmod TEXT,
    fetched INTEGER NOT NULL,
    title TEXT,
    description TEXT,
    text TEXT,
    error TEXT
);
