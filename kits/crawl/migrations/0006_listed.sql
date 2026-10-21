-- Whether a URL was listed by its program or a crawler's links; a sitemap read keeps it.

ALTER TABLE crawl_urls ADD COLUMN listed INTEGER NOT NULL DEFAULT 0;
