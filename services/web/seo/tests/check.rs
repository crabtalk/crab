use berm_lang::{http::Response, test};
use seo::{report::Code, tools};

fn page(body: &str, extra: &[(&str, &str)]) {
    let mut headers = vec![("content-type".into(), "text/html".into())];
    headers.extend(
        extra
            .iter()
            .map(|(key, value)| (key.to_string(), value.to_string())),
    );
    test::respond_with(
        "GET",
        "https://example.com/page",
        Response {
            status: 200,
            headers,
            body: body.as_bytes().to_vec(),
        },
    );
}

#[test]
fn reads_metadata_and_accepts_decorative_images_and_relative_canonicals() {
    page(
        r#"<html lang="en"><head><title>Example &amp; guide</title><meta NAME="description" content="A guide"><link rel="alternate CANONICAL" href="/page"></head><body><h1>Example</h1><img src="/icon.png" alt=""></body></html>"#,
        &[],
    );
    let report = tools::check("https://example.com/page".into()).unwrap();
    assert_eq!(report.title.as_deref(), Some("Example & guide"));
    assert_eq!(
        report.canonical.as_deref(),
        Some("https://example.com/page")
    );
    assert_eq!(report.findings.len(), 1);
    assert_eq!(report.findings[0].code, Code::MissingSocialMetadata);
    assert_eq!(report.images_without_alt, 0);
}

#[test]
fn returns_specific_findings_instead_of_a_score() {
    page(
        "<html><body><h1>One</h1><h1>Two</h1><img src='/photo'></body></html>",
        &[("X-Robots-Tag", "googlebot: noindex, nofollow")],
    );
    let report = tools::check("https://example.com/page".into()).unwrap();
    for code in [
        Code::MissingTitle,
        Code::MissingDescription,
        Code::MissingCanonical,
        Code::MissingLanguage,
        Code::MultipleH1,
        Code::MissingImageAlt,
        Code::Noindex,
    ] {
        assert!(
            report.findings.iter().any(|finding| finding.code == code),
            "{code:?}"
        );
    }
    assert_eq!(report.h1_count, 2);
    assert_eq!(report.images_without_alt, 1);
    assert!(
        berm_lang::serde_json::to_value(report)
            .unwrap()
            .get("score")
            .is_none()
    );
}

#[test]
fn ignores_fake_tags_in_scripts_and_reports_conflicting_canonicals() {
    page(
        r#"<html lang="en"><head><script>const fake = '<title>Fake</title><meta name="description" content="Fake">'</script><link rel="canonical" href="javascript:alert(1)"><link rel="canonical" href="/other"></head><body>Content</body></html>"#,
        &[],
    );
    let report = tools::check("https://example.com/page".into()).unwrap();
    for code in [
        Code::MissingTitle,
        Code::MissingDescription,
        Code::InvalidCanonical,
        Code::MultipleCanonicals,
    ] {
        assert!(report.findings.iter().any(|finding| finding.code == code));
    }
    assert!(report.canonical.is_none());
}

#[test]
fn reports_noindex_from_meta_and_bounds_heading_evidence() {
    let headings = "<h1>Section</h1>".repeat(25);
    page(
        &format!("<head><meta name='robots' content='none'></head><body>{headings}</body>"),
        &[],
    );
    let report = tools::check("https://example.com/page".into()).unwrap();
    assert!(
        report
            .findings
            .iter()
            .any(|finding| finding.code == Code::Noindex)
    );
    assert_eq!(report.h1_count, 25);
    assert_eq!(report.h1.len(), 20);
}

#[test]
fn uses_the_shared_page_complexity_limit() {
    page(&"<p>x</p>".repeat(webpage::page::MAX_NODES), &[]);
    assert!(
        tools::check("https://example.com/page".into())
            .err()
            .unwrap()
            .message
            .contains("HTML node limit")
    );
}

#[test]
fn svg_titles_are_not_page_titles() {
    for (head, expected, missing) in [
        ("<title>Page</title>", Some("Page"), false),
        ("", None, true),
    ] {
        page(
            &format!(
                "<html><head>{head}</head><body><svg><title>Icon</title></svg><svg><title>Another icon</title></svg></body></html>"
            ),
            &[],
        );
        let report = tools::check("https://example.com/page".into()).unwrap();
        assert_eq!(report.title.as_deref(), expected);
        assert_eq!(
            report
                .findings
                .iter()
                .any(|finding| finding.code == Code::MissingTitle),
            missing
        );
        assert!(
            !report
                .findings
                .iter()
                .any(|finding| finding.code == Code::MultipleTitles)
        );
    }
}

#[test]
fn duplicate_html_titles_are_still_reported() {
    page(
        "<head><title>First</title><title>Second</title></head><body><svg><title>Icon</title></svg></body>",
        &[],
    );
    let report = tools::check("https://example.com/page".into()).unwrap();
    assert_eq!(report.title.as_deref(), Some("First"));
    let finding = report
        .findings
        .iter()
        .find(|finding| finding.code == Code::MultipleTitles)
        .unwrap();
    assert_eq!(
        finding.message,
        "The returned HTML contains 2 title elements."
    );
}

#[test]
fn reads_header_canonicals_with_quoted_delimiters_and_response_base() {
    page(
        "<head><base href='https://cdn.example/assets/'></head>",
        &[(
            "Link",
            r#"</preferred?a=1,2>; title="semi; comma, quote\""; REL="canonical alternate", </next>; rel=next"#,
        )],
    );
    let report = tools::check("https://example.com/page".into()).unwrap();
    assert_eq!(
        report.canonical.as_deref(),
        Some("https://example.com/preferred?a=1,2")
    );
    assert_eq!(report.canonicals.len(), 1);
    assert!(report.link_headers[0].error.is_none());
    assert!(
        !report
            .findings
            .iter()
            .any(|finding| finding.code == Code::MissingCanonical)
    );
}

#[test]
fn exposes_canonical_conflicts_invalid_targets_body_links_and_header_context() {
    page(
        "<head><base href='/guide/'><link rel=canonical href='preferred'><link rel=canonical href='javascript:bad'></head><body><link rel=canonical href='/body'></body>",
        &[
            ("Link", "</header>; rel=canonical"),
            (
                "Link",
                "</unrelated>; rel=canonical; anchor=\"/other-page\"",
            ),
        ],
    );
    let report = tools::check("https://example.com/page".into()).unwrap();
    assert_eq!(
        report.canonical.as_deref(),
        Some("https://example.com/guide/preferred")
    );
    assert_eq!(report.canonicals.len(), 5);
    assert_eq!(report.canonicals[1].href.as_deref(), Some("javascript:bad"));
    assert_eq!(report.canonicals[1].target, None);
    for code in [
        Code::ConflictingCanonicals,
        Code::CanonicalOutsideHead,
        Code::InvalidCanonical,
        Code::MultipleCanonicals,
    ] {
        assert!(
            report.findings.iter().any(|finding| finding.code == code),
            "{code:?}"
        );
    }
}

#[test]
fn unrelated_header_context_and_body_links_do_not_supply_a_canonical() {
    page(
        "<body><link rel=canonical href='/body'></body>",
        &[("Link", "</canonical>; rel=canonical; anchor=\"/elsewhere\"")],
    );
    let report = tools::check("https://example.com/page".into()).unwrap();
    assert_eq!(report.canonical, None);
    assert!(
        report
            .findings
            .iter()
            .any(|finding| finding.code == Code::MissingCanonical)
    );
}

#[test]
fn preserves_malformed_link_header_evidence() {
    page(
        "<body>Content</body>",
        &[("Link", "</target>; rel=\"canonical")],
    );
    let report = tools::check("https://example.com/page".into()).unwrap();
    assert!(report.link_headers[0].error.is_some());
    assert_eq!(report.link_headers[0].value, "</target>; rel=\"canonical");
    assert!(
        report
            .findings
            .iter()
            .any(|finding| finding.code == Code::InvalidLinkHeader)
    );
}

#[test]
fn inspects_json_ld_arrays_graphs_nested_types_and_malformed_blocks() {
    page(
        r#"<head>
    <script type="APPLICATION/LD+JSON">[{"@context":{"author":{"@id":"https://schema.org/author","@type":"@id"}},"@graph":[{"@type":["Article","CreativeWork"],"author":{"@type":"Person"}}]}]</script>
    <script type="application/ld+json">{"@type":</script>
    <script type="application/ld+json">{"@type":12}</script>
    <script type="application/ld+json">[3]</script>
    <script type="application/ld+json">{"@graph":[false]}</script>
    <script type="application/json">not JSON-LD</script>
    </head>"#,
        &[],
    );
    let report = tools::check("https://example.com/page".into()).unwrap();
    assert_eq!(report.structured_data.len(), 5);
    assert_eq!(
        report.structured_data[0].types,
        ["Article", "CreativeWork", "Person"]
    );
    assert!(report.structured_data[0].error.is_none());
    assert_eq!(report.structured_data[1].index, 2);
    assert!(
        report.structured_data[1..]
            .iter()
            .all(|block| block.error.is_some())
    );
    assert!(
        report
            .findings
            .iter()
            .any(|finding| finding.code == Code::InvalidStructuredData)
    );
}

#[test]
fn accepts_open_graph_arrays_and_twitter_fallbacks() {
    page(
        r#"<head>
    <meta property="og:title" content="Page"><meta property="og:type" content="website">
    <meta property="og:description" content="Description"><meta property="og:url" content="https://example.com/page">
    <meta property="og:image" content="https://example.com/one.png"><meta property="og:image" content="https://example.com/two.png">
    <meta name="twitter:card" content="summary_large_image"></head>"#,
        &[],
    );
    let report = tools::check("https://example.com/page".into()).unwrap();
    assert!(report.social.missing.is_empty());
    assert!(report.social.invalid_urls.is_empty());
    assert!(report.social.conflicting.is_empty());
    assert_eq!(report.social.properties.len(), 7);
}

#[test]
fn exposes_missing_conflicting_and_invalid_social_metadata() {
    page(
        r#"<head><meta property="og:title" content="One"><meta property="og:title" content="Two">
    <meta property="og:image" content="/relative.png"><meta name="twitter:image" content="javascript:bad">
    <meta name="twitter:card" content="unknown"></head>"#,
        &[],
    );
    let report = tools::check("https://example.com/page".into()).unwrap();
    for code in [
        Code::MissingSocialMetadata,
        Code::InvalidSocialUrl,
        Code::InvalidTwitterCard,
        Code::ConflictingSocialMetadata,
    ] {
        assert!(
            report.findings.iter().any(|finding| finding.code == code),
            "{code:?}"
        );
    }
    assert!(report.social.missing.iter().any(|name| name == "og:type"));
    assert_eq!(report.social.invalid_urls, ["og:image", "twitter:image"]);
    assert_eq!(report.social.conflicting, ["og:title"]);
}

#[test]
fn player_cards_report_missing_player_dimensions_and_url() {
    page(
        "<head><meta name='twitter:card' content='player'></head>",
        &[],
    );
    let report = tools::check("https://example.com/page".into()).unwrap();
    for name in [
        "twitter:player",
        "twitter:player:width",
        "twitter:player:height",
    ] {
        assert!(report.social.missing.iter().any(|missing| missing == name));
    }
}

#[test]
fn retains_duplicate_descriptions_and_bounds_image_evidence() {
    let images = (0..25)
        .map(|index| format!("<img src='/image-{index}' srcset='/large-{index} 2x'>"))
        .collect::<String>();
    page(
        &format!(
            "<head><meta name=description content='First'><meta name=description content=''><meta name=description></head><body><img src='/decorative' alt=''>{images}</body>"
        ),
        &[],
    );
    let report = tools::check("https://example.com/page".into()).unwrap();
    assert_eq!(report.description.as_deref(), Some("First"));
    assert_eq!(
        report.descriptions,
        [Some("First".into()), Some("".into()), None]
    );
    assert!(
        report
            .findings
            .iter()
            .any(|finding| finding.code == Code::MultipleDescriptions)
    );
    assert_eq!(report.images, 26);
    assert_eq!(report.images_without_alt, 25);
    assert_eq!(report.image_alt_samples.len(), 20);
    assert_eq!(report.image_alt_samples[0].index, 2);
    assert_eq!(report.image_alt_samples[0].src.as_deref(), Some("/image-0"));
    assert_eq!(
        report.image_alt_samples[0].srcset.as_deref(),
        Some("/large-0 2x")
    );
}

#[test]
fn retains_robots_source_crawler_and_scope_across_combined_headers() {
    use seo::robots::Source;
    page(
        "<head><meta name=robots content=follow><meta name=googlebot-news content=noindex><meta name=bingbot content=none></head>",
        &[
            (
                "X-Robots-Tag",
                "otherbot: noindex, nofollow, googlebot: max-snippet: 20, noindex",
            ),
            ("X-Robots-Tag", "noindex"),
        ],
    );
    let report = tools::check("https://example.com/page".into()).unwrap();
    assert_eq!(report.robots.len(), 5);
    assert_eq!(report.robots_directives.len(), 8);
    assert!(matches!(
        report.robots_directives[1].source,
        Source::HtmlMeta
    ));
    assert_eq!(
        report.robots_directives[1].crawler.as_deref(),
        Some("googlebot-news")
    );
    assert!(matches!(
        report.robots_directives[3].source,
        Source::HttpHeader
    ));
    assert_eq!(
        report.robots_directives[4].crawler.as_deref(),
        Some("otherbot")
    );
    assert_eq!(
        report.robots_directives[5].crawler.as_deref(),
        Some("googlebot")
    );
    assert_eq!(report.robots_directives[7].crawler, None);
    let findings = report
        .findings
        .iter()
        .filter(|finding| finding.code == Code::Noindex)
        .collect::<Vec<_>>();
    assert_eq!(findings.len(), 5);
    for scope in [
        "googlebot-news",
        "bingbot",
        "otherbot",
        "googlebot",
        "all crawlers",
    ] {
        assert!(
            findings
                .iter()
                .any(|finding| finding.message.contains(scope)),
            "{scope}"
        );
    }
}

#[test]
fn does_not_read_noindex_from_a_parameter_value_or_other_metadata() {
    page(
        "<head><meta name=description content=noindex><meta name=robots content='unavailable_after: noindex'></head>",
        &[("X-Robots-Tag", "googlebot: max-snippet: 0, nofollow")],
    );
    let report = tools::check("https://example.com/page".into()).unwrap();
    assert!(
        !report
            .findings
            .iter()
            .any(|finding| finding.code == Code::Noindex)
    );
}

#[test]
fn validates_hreflang_languages_absolute_targets_and_conflicts() {
    page(
        "<head><base href='https://cdn.example/'>\
        <link rel=alternate hreflang=en-US href='https://example.com/us'>\
        <link rel=alternate hreflang=EN-us href='https://example.com/other'>\
        <link rel=alternate hreflang=en-UK href='https://example.com/uk'>\
        <link rel=alternate hreflang=zz href='https://example.com/zz'>\
        <link rel=alternate hreflang=fr href='/fr'>\
        <link rel=alternate hreflang=de href='javascript:bad'>\
        <link rel=alternate hreflang=x-default href='https://example.com/'>\
        <link rel=alternate hreflang=zh-Hant-TW href='https://example.com/tw'>\
        <link rel=alternate hreflang=fr-FR href='//example.com/fr'>\
        </head>",
        &[],
    );
    let report = tools::check("https://example.com/page".into()).unwrap();
    assert_eq!(report.hreflang.len(), 9);
    assert!(report.hreflang[0].valid_language);
    assert!(!report.hreflang[2].valid_language);
    assert!(!report.hreflang[3].valid_language);
    assert!(report.hreflang[4].target.is_none());
    assert_eq!(report.hreflang[4].href.as_deref(), Some("/fr"));
    assert!(report.hreflang[5].target.is_none());
    assert!(report.hreflang[6].valid_language);
    assert!(report.hreflang[7].valid_language);
    assert!(report.hreflang[8].target.is_none());
    for code in [
        Code::InvalidHreflangLanguage,
        Code::InvalidHreflangUrl,
        Code::ConflictingHreflang,
    ] {
        assert!(
            report.findings.iter().any(|finding| finding.code == code),
            "{code:?}"
        );
    }
}

#[test]
fn repeated_identical_hreflang_targets_are_not_conflicts() {
    page(
        "<head><link rel=alternate hreflang=en href='https://example.com/en'><link rel=alternate hreflang=EN href='https://example.com/en'></head>",
        &[],
    );
    let report = tools::check("https://example.com/page".into()).unwrap();
    assert!(
        !report
            .findings
            .iter()
            .any(|finding| finding.code == Code::ConflictingHreflang)
    );
}

#[test]
fn expanded_evidence_remains_subject_to_report_size_limit() {
    page(
        &format!(
            "<head>{}</head>",
            format!("<meta name='description' content='{}'>", "x".repeat(120)).repeat(2300)
        ),
        &[],
    );
    assert!(
        tools::check("https://example.com/page".into())
            .err()
            .unwrap()
            .message
            .contains("report limit")
    );
}

#[test]
fn identical_html_and_header_canonicals_do_not_conflict() {
    page(
        "<head><link rel=canonical href='/page'></head>",
        &[("Link", "</page>; rel=canonical")],
    );
    let report = tools::check("https://example.com/page".into()).unwrap();
    assert_eq!(
        report.canonical.as_deref(),
        Some("https://example.com/page")
    );
    assert!(
        !report
            .findings
            .iter()
            .any(|finding| finding.code == Code::ConflictingCanonicals)
    );
}

#[test]
fn robots_dates_do_not_become_crawler_names() {
    page(
        "<body>Content</body>",
        &[(
            "X-Robots-Tag",
            "googlebot: unavailable_after: Wed, 03 Dec 2025 13:00:00 GMT, noindex",
        )],
    );
    let report = tools::check("https://example.com/page".into()).unwrap();
    assert!(
        report
            .robots_directives
            .iter()
            .all(|directive| directive.crawler.as_deref() == Some("googlebot"))
    );
    assert_eq!(
        report
            .findings
            .iter()
            .filter(|finding| finding.code == Code::Noindex)
            .count(),
        1
    );
}
