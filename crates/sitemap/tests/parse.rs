use crab_sitemap::Sitemap;

#[test]
fn reads_index_and_page_urls_with_their_own_metadata() {
    let index = Sitemap::parse(
        r#"<?xml version="1.0"?><sitemapindex xmlns="http://www.sitemaps.org/schemas/sitemap/0.9"><sitemap><loc>https://example.org/docs.xml</loc></sitemap></sitemapindex>"#,
    );
    assert!(index.complete);
    assert_eq!(index.sitemaps, ["https://example.org/docs.xml"]);
    let pages = Sitemap::parse(
        r#"<urlset xmlns:image="http://www.google.com/schemas/sitemap-image/1.1"><url><loc>https://example.org/?a=1&amp;b=2</loc><image:image><image:loc>https://example.org/image.png</image:loc></image:image><lastmod>2026-11-19</lastmod></url></urlset>"#,
    );
    assert!(pages.complete);
    assert_eq!(
        pages.urls,
        [(
            "https://example.org/?a=1&b=2".into(),
            Some("2026-11-19".into())
        )]
    );
}

#[test]
fn partial_or_non_sitemap_xml_does_not_prove_membership() {
    let partial = Sitemap::parse("<urlset><url><loc>https://example.org/a</loc></url>");
    assert!(!partial.complete);
    assert_eq!(partial.urls.len(), 1);
    assert!(!Sitemap::parse("<html><body>Page not found</body></html>").complete);
    assert!(!Sitemap::parse("<urlset><url><loc></bad></url></urlset>").complete);
}
