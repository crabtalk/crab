-- What a failed read or fetch answered: a site's last error and when it failed, and a page's last error.

ALTER TABLE crawl_sites ADD COLUMN error TEXT;
ALTER TABLE crawl_sites ADD COLUMN failed INTEGER NOT NULL DEFAULT 0;
ALTER TABLE crawl_pages ADD COLUMN error TEXT;

CREATE INDEX crawl_sites_failures ON crawl_sites (failures);
