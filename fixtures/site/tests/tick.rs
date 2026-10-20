//! `crab-crawl`'s tick in berm-site, against an arranged site: robots.txt
//! rules and sitemaps read, pages of a kind fetched and recrawled when due, and
//! a failed read logged and retried.

use anyhow::Result;
use berm_api::Output;
use berm_host::test::{self, TestHost};
use serde_json::{Value, json};

/// 2026-10-12T00:00:00Z.
const NOW: u64 = 1_791_763_200_000;

const DAY: u64 = 24 * 60 * 60 * 1000;

const ROBOTS: &str = "\
User-agent: *
Disallow: /compare/secret
Content-Signal: search=yes, ai-train=no
Sitemap: https://example.com/sitemap-index.xml

User-agent: other-bot
Disallow: /
";

const INDEX: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<sitemapindex xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
  <sitemap><loc>https://example.com/sitemap-pages.xml</loc></sitemap>
</sitemapindex>"#;

const PAGES: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
  <url><loc>https://example.com/pricing</loc><lastmod>2026-10-01</lastmod></url>
  <url><loc>https://example.com/compare/example-vs-other</loc></url>
  <url><loc>https://example.com/compare/secret</loc></url>
  <url><loc>https://example.com/blog/hello</loc></url>
  <url><loc>https://elsewhere.com/pricing</loc></url>
</urlset>"#;

const PRICING: &str = r#"<html><head><title>Pricing &amp; plans</title>
<meta name="description" content="What Example costs.">
<script>var tracking = "not text";</script></head>
<body><h1>Plans</h1><p>Free, then $10 a month.</p></body></html>"#;

const COMPARE: &str = r#"<html><head><meta property="og:title" content="Example vs Other"></head>
<body><div id="app"></div><script src="/app.js"></script></body></html>"#;

async fn deployed() -> Result<TestHost> {
    let host = TestHost::open(NOW).await?;
    host.deploy("site", &test::build("berm-site", "site")?)
        .await?;
    Ok(host)
}

fn arrange(host: &TestHost) {
    host.arrange("GET", "https://example.com/robots.txt", 200, ROBOTS);
    host.arrange("GET", "https://example.com/sitemap-index.xml", 200, INDEX);
    host.arrange("GET", "https://example.com/sitemap-pages.xml", 200, PAGES);
    host.arrange("GET", "https://example.com/pricing", 200, PRICING);
    host.arrange(
        "GET",
        "https://example.com/compare/example-vs-other",
        200,
        COMPARE,
    );
}

async fn call(host: &TestHost, tool: &str, args: Value) -> Result<Value> {
    let ran = host
        .call("site", tool, serde_json::to_vec(&args)?.as_slice())
        .await?;
    let Output::Done(result) = ran.result else {
        panic!("{tool} runs: {:?}", ran.result);
    };
    Ok(if result.is_empty() {
        Value::Null
    } else {
        serde_json::from_str(&result)?
    })
}

/// The URLs fetched so far, in order.
fn fetched(host: &TestHost) -> Vec<String> {
    host.fetched()
        .into_iter()
        .map(|request| request.url)
        .collect()
}

#[tokio::test]
async fn a_tick_keeps_the_pages_of_a_kind_robots_allows() -> Result<()> {
    let host = deployed().await?;
    arrange(&host);
    assert_eq!(
        call(&host, "watch", json!({"domains": ["Example.com"]})).await?,
        1
    );
    assert_eq!(
        call(&host, "watch", json!({"domains": ["example.com"]})).await?,
        0
    );
    call(&host, "tick", json!({})).await?;

    let urls = call(&host, "urls", json!({"domain": "example.com"})).await?;
    assert_eq!(
        urls,
        json!([
            {"url": "https://example.com/compare/example-vs-other", "kind": "comparison"},
            {"url": "https://example.com/pricing", "kind": "pricing", "lastmod": "2026-10-01"},
        ])
    );

    let pricing = call(&host, "page", json!({"url": "https://example.com/pricing"})).await?;
    assert_eq!(pricing["status"], 200);
    assert_eq!(pricing["title"], "Pricing & plans");
    assert_eq!(pricing["description"], "What Example costs.");
    let text = pricing["text"].as_str().unwrap_or_default();
    assert!(text.contains("Free, then $10 a month."), "{text}");
    assert!(!text.contains("tracking"), "{text}");

    // A page rendered by JavaScript is kept without text.
    let compare = call(
        &host,
        "page",
        json!({"url": "https://example.com/compare/example-vs-other"}),
    )
    .await?;
    assert_eq!(compare["title"], "Example vs Other");
    assert!(compare.get("text").is_none(), "{compare}");

    let site = host
        .sql(
            "site",
            "SELECT signal, read FROM crawl_sites WHERE domain = 'example.com'",
            json!([]),
        )
        .await?;
    assert_eq!(site["rows"][0], json!(["search=yes, ai-train=no", NOW]));
    assert_eq!(host.logs("site", Some("Read")).await?.len(), 1);
    Ok(())
}

#[tokio::test]
async fn pages_are_fetched_again_only_when_due() -> Result<()> {
    let host = deployed().await?;
    arrange(&host);
    call(&host, "watch", json!({"domains": ["example.com"]})).await?;
    call(&host, "tick", json!({})).await?;
    let first = fetched(&host).len();

    // Nothing is due a day later.
    host.advance(DAY);
    call(&host, "tick", json!({})).await?;
    assert_eq!(fetched(&host).len(), first);

    // A week on, the site is read again; neither page is due.
    host.advance(7 * DAY);
    call(&host, "tick", json!({})).await?;
    let reread = &fetched(&host)[first..];
    assert_eq!(
        reread,
        [
            "https://example.com/robots.txt",
            "https://example.com/sitemap-index.xml",
            "https://example.com/sitemap-pages.xml",
        ]
    );

    // A changed lastmod fetches the page again, as does a month for the page
    // with none.
    host.arrange(
        "GET",
        "https://example.com/sitemap-pages.xml",
        200,
        PAGES.replace("2026-10-01", "2026-10-20"),
    );
    host.advance(30 * DAY);
    let before = fetched(&host).len();
    call(&host, "tick", json!({})).await?;
    let again: Vec<String> = fetched(&host)[before..].to_vec();
    assert!(
        again.contains(&"https://example.com/pricing".to_owned()),
        "{again:?}"
    );
    assert!(
        again.contains(&"https://example.com/compare/example-vs-other".to_owned()),
        "{again:?}"
    );
    Ok(())
}

#[tokio::test]
async fn a_failed_read_is_logged_and_retried_up_to_the_cap() -> Result<()> {
    let host = deployed().await?;
    host.arrange("GET", "https://example.com/robots.txt", 503, "unavailable");
    call(&host, "watch", json!({"domains": ["example.com"]})).await?;

    call(&host, "tick", json!({})).await?;
    call(&host, "tick", json!({})).await?;
    let failed = host.logs("site", Some("ReadFailed")).await?;
    assert_eq!(failed.len(), 2);
    let error: Value = serde_json::from_str(&failed[0].data)?;
    assert_eq!(
        error["error"],
        "https://example.com/robots.txt answered 503"
    );

    // Two failures in a row is this crawler's cap: the site is read no more.
    call(&host, "tick", json!({})).await?;
    assert_eq!(host.logs("site", Some("ReadFailed")).await?.len(), 2);
    Ok(())
}
