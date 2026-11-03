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
    assert!(report.findings.is_empty());
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
