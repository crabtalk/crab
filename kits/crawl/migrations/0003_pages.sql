-- Each page fetched: its status, the validators it answered with, the lastmod its URL had when fetched, and its title, description and text.

CREATE TABLE crawl_pages (
    url TEXT PRIMARY KEY,
    status INTEGER NOT NULL,
    etag TEXT,
    modified TEXT,
    lastmod TEXT,
    fetched INTEGER NOT NULL,
    title TEXT,
    description TEXT,
    text TEXT
);

CREATE INDEX crawl_pages_fetched ON crawl_pages (fetched);
