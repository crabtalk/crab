-- Each URL its program or a crawler's links listed, beside its sitemaps': whether it was listed, and the most bytes of HTML it is read for, absent for the schedule's page bytes.

ALTER TABLE crawl_urls ADD COLUMN listed INTEGER NOT NULL DEFAULT 0;
ALTER TABLE crawl_urls ADD COLUMN bytes INTEGER;

DROP INDEX crawl_sites_read;
DROP INDEX crawl_sites_failures;
DROP INDEX crawl_pages_fetched;
CREATE INDEX crawl_sites_failed ON crawl_sites (failed);
