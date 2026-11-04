-- Whether a site is read before the other sites due: a program's own sources.

ALTER TABLE crawl_sites ADD COLUMN first INTEGER NOT NULL DEFAULT 0;

CREATE INDEX crawl_sites_first ON crawl_sites (first, read);
