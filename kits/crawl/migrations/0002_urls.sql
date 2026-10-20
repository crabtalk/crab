-- Each URL a site's sitemaps list that is of a kind its program names, with the sitemap's lastmod as written.

CREATE TABLE crawl_urls (
    url TEXT PRIMARY KEY,
    domain TEXT NOT NULL,
    kind TEXT NOT NULL,
    lastmod TEXT
);

CREATE INDEX crawl_urls_domain ON crawl_urls (domain, kind);
