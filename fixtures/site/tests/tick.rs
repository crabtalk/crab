//! `crab-crawl`'s tick in berm-site, against an arranged site: robots.txt
//! rules and sitemaps read, pages of a kind fetched, handed to the crawler and
//! recrawled when due, sites watched first read and fetched first, a page of
//! each site before a second of any, a silent site fetched no more in its
//! tick, a page cut at the bytes it is read for, a failed read logged and
//! retried, the URLs a program lists and the links its crawler lists fetched
//! under the site's robots.txt, and a URL listed with its bytes read for them,
//! past the tick's HTML when it is the tick's first, and the sites after a
//! tick's first sharing its sitemap bytes.

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
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9" xmlns:image="http://www.google.com/schemas/sitemap-image/1.1">
  <url><loc>https://example.com/pricing</loc><image:image><image:loc>https://example.com/plans.png</image:loc></image:image><lastmod>2026-10-01</lastmod></url>
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

    // Each page that answered with HTML is handed to the crawler.
    assert_eq!(
        call(&host, "read", json!({"url": "https://example.com/pricing"})).await?,
        json!({"kind": "pricing", "length": PRICING.len()})
    );
    assert_eq!(
        call(
            &host,
            "read",
            json!({"url": "https://example.com/compare/example-vs-other"})
        )
        .await?,
        json!({"kind": "comparison", "length": COMPARE.len()})
    );

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
async fn a_failed_read_is_kept_and_retried_up_to_the_cap_then_a_week_on() -> Result<()> {
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

    // Two failures in a row is this crawler's cap: the site keeps its last
    // error, and is not read again until a week after its last failure.
    call(&host, "tick", json!({})).await?;
    assert_eq!(host.logs("site", Some("ReadFailed")).await?.len(), 2);
    let site = host
        .sql(
            "site",
            "SELECT failures, error, failed FROM crawl_sites WHERE domain = 'example.com'",
            json!([]),
        )
        .await?;
    assert_eq!(
        site["rows"][0],
        json!([2, "https://example.com/robots.txt answered 503", NOW])
    );
    host.advance(7 * DAY);
    call(&host, "tick", json!({})).await?;
    assert_eq!(host.logs("site", Some("ReadFailed")).await?.len(), 3);
    Ok(())
}

#[tokio::test]
async fn a_site_watched_first_is_read_first() -> Result<()> {
    let host = deployed().await?;
    call(&host, "watch", json!({"domains": ["elsewhere.com"]})).await?;
    call(&host, "watch_first", json!({"domains": ["example.com"]})).await?;
    call(&host, "tick", json!({})).await?;
    assert_eq!(
        fetched(&host).first().map(String::as_str),
        Some("https://example.com/robots.txt")
    );
    Ok(())
}

/// elsewhere.com, its sitemap listing twelve comparison pages, none of them
/// arranged, and read in a tick: its first page answers nothing, the rest are
/// left due.
async fn elsewhere_read(host: &TestHost) -> Result<()> {
    host.arrange(
        "GET",
        "https://elsewhere.com/robots.txt",
        200,
        "User-agent: *\nAllow: /\n",
    );
    let urls: String = (0..12)
        .map(|n| format!("<url><loc>https://elsewhere.com/compare/elsewhere-vs-{n}</loc></url>"))
        .collect();
    host.arrange(
        "GET",
        "https://elsewhere.com/sitemap.xml",
        200,
        format!(r#"<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">{urls}</urlset>"#),
    );
    call(host, "watch", json!({"domains": ["elsewhere.com"]})).await?;
    call(host, "tick", json!({})).await?;
    Ok(())
}

/// The pages a tick fetches.
async fn ticked_pages(host: &TestHost) -> Result<Vec<String>> {
    let before = fetched(host).len();
    call(host, "tick", json!({})).await?;
    Ok(fetched(host)[before..]
        .iter()
        .filter(|url| !url.ends_with(".xml") && !url.ends_with(".txt"))
        .cloned()
        .collect())
}

#[tokio::test]
async fn the_pages_of_a_site_watched_first_are_fetched_first() -> Result<()> {
    let host = deployed().await?;
    elsewhere_read(&host).await?;
    arrange(&host);
    call(&host, "watch_first", json!({"domains": ["example.com"]})).await?;
    let pages = ticked_pages(&host).await?;
    assert!(pages[0].starts_with("https://example.com/"), "{pages:?}");
    Ok(())
}

#[tokio::test]
async fn a_tick_takes_a_page_of_each_site_due_before_a_second_of_any() -> Result<()> {
    let host = deployed().await?;
    elsewhere_read(&host).await?;
    arrange(&host);
    call(&host, "watch", json!({"domains": ["example.com"]})).await?;
    let pages = ticked_pages(&host).await?;
    assert!(
        pages
            .iter()
            .any(|url| url.starts_with("https://example.com/")),
        "{pages:?}"
    );
    Ok(())
}

#[tokio::test]
async fn a_site_that_answers_a_page_with_nothing_is_fetched_no_more_that_tick() -> Result<()> {
    let host = deployed().await?;
    host.arrange("GET", "https://example.com/robots.txt", 200, ROBOTS);
    host.arrange("GET", "https://example.com/sitemap-index.xml", 200, INDEX);
    host.arrange("GET", "https://example.com/sitemap-pages.xml", 200, PAGES);
    call(&host, "watch", json!({"domains": ["example.com"]})).await?;
    call(&host, "tick", json!({})).await?;
    // Neither page is arranged: the first answers nothing, the second is left.
    let pages: Vec<String> = fetched(&host)
        .into_iter()
        .filter(|url| !url.ends_with(".xml") && !url.ends_with(".txt"))
        .collect();
    assert_eq!(pages.len(), 1, "{pages:?}");
    assert_eq!(host.logs("site", Some("FetchFailed")).await?.len(), 1);
    Ok(())
}

#[tokio::test]
async fn a_page_reaches_the_crawler_cut_at_the_bytes_a_page_is_read_for() -> Result<()> {
    let host = deployed().await?;
    arrange(&host);
    let long = format!("<html><body><p>{}</p></body></html>", "a".repeat(300 << 10));
    host.arrange("GET", "https://example.com/pricing", 200, long);
    call(&host, "watch", json!({"domains": ["example.com"]})).await?;
    call(&host, "tick", json!({})).await?;
    assert_eq!(
        call(&host, "read", json!({"url": "https://example.com/pricing"})).await?,
        json!({"kind": "pricing", "length": 256 << 10})
    );
    Ok(())
}

/// The URLs `domain`'s site keeps.
async fn kept(host: &TestHost, domain: &str) -> Result<Vec<String>> {
    let urls = call(host, "urls", json!({ "domain": domain })).await?;
    Ok(urls
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|listed| listed["url"].as_str().map(String::from))
        .collect())
}

#[tokio::test]
async fn a_url_its_program_lists_is_fetched_once_its_site_is_read_and_kept_by_its_next_read()
-> Result<()> {
    let host = deployed().await?;
    arrange(&host);
    let listed = "https://example.com/compare/listed-vs-other";
    host.arrange("GET", listed, 200, COMPARE);
    call(&host, "watch", json!({"domains": ["example.com"]})).await?;
    let new = call(
        &host,
        "list",
        json!({"domain": "example.com", "urls": [listed, "https://example.com/blog/hello", "https://elsewhere.com/pricing"]}),
    )
    .await?;
    assert_eq!(new, 1);
    call(&host, "tick", json!({})).await?;
    assert!(fetched(&host).contains(&listed.to_owned()));

    // A week on, the site's sitemaps are read again and the listed URL stays.
    host.advance(7 * DAY);
    call(&host, "tick", json!({})).await?;
    assert!(
        kept(&host, "example.com")
            .await?
            .contains(&listed.to_owned())
    );
    Ok(())
}

#[tokio::test]
async fn a_listed_url_its_robots_txt_disallows_is_not_fetched() -> Result<()> {
    let host = deployed().await?;
    arrange(&host);
    let secret = "https://example.com/compare/secret";
    call(&host, "watch", json!({"domains": ["example.com"]})).await?;
    call(
        &host,
        "list",
        json!({"domain": "example.com", "urls": [secret]}),
    )
    .await?;
    call(&host, "tick", json!({})).await?;
    assert!(!fetched(&host).contains(&secret.to_owned()));
    assert!(
        !kept(&host, "example.com")
            .await?
            .contains(&secret.to_owned())
    );
    Ok(())
}

#[tokio::test]
async fn a_url_listed_after_its_site_is_read_is_held_to_its_robots_txt() -> Result<()> {
    let host = deployed().await?;
    arrange(&host);
    let secret = "https://example.com/compare/secret";
    call(&host, "watch", json!({"domains": ["example.com"]})).await?;
    call(&host, "tick", json!({})).await?;
    assert_eq!(
        call(
            &host,
            "list",
            json!({"domain": "example.com", "urls": [secret]}),
        )
        .await?,
        json!(0)
    );
    call(&host, "tick", json!({})).await?;
    assert!(!fetched(&host).contains(&secret.to_owned()));
    Ok(())
}

#[tokio::test]
async fn a_robots_txt_opening_with_a_byte_order_mark_keeps_its_first_group() -> Result<()> {
    let host = deployed().await?;
    arrange(&host);
    host.arrange(
        "GET",
        "https://example.com/robots.txt",
        200,
        format!("\u{feff}{ROBOTS}"),
    );
    call(&host, "watch", json!({"domains": ["example.com"]})).await?;
    call(&host, "tick", json!({})).await?;
    assert!(
        !kept(&host, "example.com")
            .await?
            .contains(&"https://example.com/compare/secret".to_owned())
    );
    Ok(())
}

#[tokio::test]
async fn a_pattern_of_many_wildcards_is_matched_in_time() -> Result<()> {
    let host = deployed().await?;
    arrange(&host);
    let stars = "*a".repeat(30);
    host.arrange(
        "GET",
        "https://example.com/robots.txt",
        200,
        format!("User-agent: *\nDisallow: /compare/{stars}b\nSitemap: https://example.com/sitemap-index.xml\n"),
    );
    let long = format!("https://example.com/compare/{}", "a".repeat(200));
    host.arrange(
        "GET",
        "https://example.com/sitemap-pages.xml",
        200,
        format!(r#"<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9"><url><loc>{long}</loc></url></urlset>"#),
    );
    call(&host, "watch", json!({"domains": ["example.com"]})).await?;
    call(&host, "tick", json!({})).await?;
    assert!(kept(&host, "example.com").await?.contains(&long));
    Ok(())
}

#[tokio::test]
async fn a_link_its_crawler_lists_is_fetched_on_a_later_tick() -> Result<()> {
    let host = deployed().await?;
    arrange(&host);
    let linked = "https://example.com/compare/linked-vs-other";
    host.arrange(
        "GET",
        "https://example.com/pricing",
        200,
        r#"<html><body><a href="/compare/linked-vs-other">Compared</a> <a href="/blog/hello">Blog</a> <a href="https://elsewhere.com/compare/a-vs-b">Elsewhere</a></body></html>"#,
    );
    host.arrange("GET", linked, 200, COMPARE);
    call(&host, "watch", json!({"domains": ["example.com"]})).await?;
    call(&host, "tick", json!({})).await?;
    assert!(!fetched(&host).contains(&linked.to_owned()));
    assert_eq!(
        kept(&host, "example.com").await?,
        [
            "https://example.com/compare/example-vs-other",
            linked,
            "https://example.com/pricing"
        ]
    );
    call(&host, "tick", json!({})).await?;
    assert!(fetched(&host).contains(&linked.to_owned()));
    Ok(())
}

#[tokio::test]
async fn a_url_listed_with_its_bytes_is_read_past_the_bytes_a_page_is_read_for() -> Result<()> {
    let host = deployed().await?;
    arrange(&host);
    let listed = "https://example.com/compare/listed-vs-other";
    let long = format!("<html><body><p>{}</p></body></html>", "a".repeat(600 << 10));
    host.arrange("GET", listed, 200, long.clone());
    call(&host, "watch", json!({"domains": ["example.com"]})).await?;
    call(
        &host,
        "list",
        json!({"domain": "example.com", "urls": [listed], "bytes": 1 << 20}),
    )
    .await?;
    call(&host, "tick", json!({})).await?;
    assert_eq!(
        call(&host, "read", json!({ "url": listed })).await?,
        json!({"kind": "comparison", "length": long.len()})
    );
    Ok(())
}

#[tokio::test]
async fn a_url_listed_past_the_html_left_is_left_due_and_read_first_on_a_tick_of_its_own()
-> Result<()> {
    let host = deployed().await?;
    arrange(&host);
    host.arrange(
        "GET",
        "https://big.com/robots.txt",
        200,
        "User-agent: *\nAllow: /\n",
    );
    host.arrange(
        "GET",
        "https://big.com/sitemap.xml",
        200,
        r#"<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9"></urlset>"#,
    );
    let big = "https://big.com/compare/big-vs-other";
    let long = format!("<html><body><p>{}</p></body></html>", "a".repeat(5 << 19));
    host.arrange("GET", big, 200, long.clone());
    call(&host, "watch_first", json!({"domains": ["example.com"]})).await?;
    call(&host, "watch", json!({"domains": ["big.com"]})).await?;
    call(
        &host,
        "list",
        json!({"domain": "big.com", "urls": [big], "bytes": 3 << 20}),
    )
    .await?;
    call(&host, "tick", json!({})).await?;
    assert!(fetched(&host).contains(&"https://example.com/pricing".to_owned()));
    assert!(!fetched(&host).contains(&big.to_owned()));

    assert_eq!(ticked_pages(&host).await?, [big]);
    assert_eq!(
        call(&host, "read", json!({ "url": big })).await?,
        json!({"kind": "comparison", "length": long.len()})
    );
    Ok(())
}

#[tokio::test]
async fn the_sites_after_the_first_share_the_sitemap_bytes_of_a_round() -> Result<()> {
    let host = deployed().await?;
    arrange(&host);
    for domain in ["a.com", "b.com"] {
        host.arrange(
            "GET",
            &format!("https://{domain}/robots.txt"),
            200,
            "User-agent: *\nAllow: /\n",
        );
        // 3 MiB each: either fits the 4 MiB the rest share, not both.
        host.arrange(
            "GET",
            &format!("https://{domain}/sitemap.xml"),
            200,
            format!(
                r#"<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9"><url><loc>https://{domain}/pricing</loc></url>{}</urlset>"#,
                " ".repeat(3 << 20)
            ),
        );
    }
    call(&host, "watch_first", json!({"domains": ["example.com"]})).await?;
    call(&host, "watch", json!({"domains": ["a.com", "b.com"]})).await?;
    call(&host, "tick", json!({})).await?;
    assert!(kept(&host, "a.com").await?.is_empty());
    assert!(kept(&host, "b.com").await?.is_empty());

    // Left due, each is read on the next tick, a.com for bytes of its own.
    call(&host, "tick", json!({})).await?;
    assert_eq!(kept(&host, "a.com").await?, ["https://a.com/pricing"]);
    assert_eq!(kept(&host, "b.com").await?, ["https://b.com/pricing"]);
    Ok(())
}
