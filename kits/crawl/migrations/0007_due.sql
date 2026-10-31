-- When each URL is next due: 0 when never fetched or its lastmod changed since, the time its page was fetched when it has no lastmod, and NULL until its lastmod changes.

ALTER TABLE crawl_urls ADD COLUMN due INTEGER DEFAULT 0;

UPDATE crawl_urls SET due = (SELECT CASE WHEN crawl_urls.lastmod IS NULL THEN p.fetched
                                         WHEN crawl_urls.lastmod IS NOT p.lastmod THEN 0 END
                               FROM crawl_pages p WHERE p.url = crawl_urls.url)
 WHERE EXISTS (SELECT 1 FROM crawl_pages p WHERE p.url = crawl_urls.url);

CREATE INDEX crawl_urls_due ON crawl_urls (domain, due);
