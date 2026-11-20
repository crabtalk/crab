use berm_lang::{http::Response, test};
use seo::{
    access::Code,
    crawlers::{Robots, Rule},
    tools,
};

fn respond(url: &str, status: u16, kind: &str, body: &str) {
    test::respond_with(
        "GET",
        url,
        Response {
            status,
            headers: vec![("content-type".into(), kind.into())],
            body: body.as_bytes().to_vec(),
        },
    );
}

#[test]
fn reads_crawler_rules_llms_txt_and_markdown_copies() {
    let words = "Example guide content for people and crawlers alike. ".repeat(8);
    respond(
        "https://example.com/",
        200,
        "text/html",
        &format!(
            r#"<html lang="en"><head><title>Example</title></head><body><h1>Example</h1><p>{words}</p></body></html>"#
        ),
    );
    respond(
        "https://example.com/robots.txt",
        200,
        "text/plain",
        "User-agent: GPTBot\nDisallow: /\n\nUser-agent: ClaudeBot\nAllow: /\n\nUser-agent: *\nDisallow: /private\n",
    );
    respond(
        "https://example.com/llms.txt",
        200,
        "text/plain",
        "# Example\n\n> A site.\n\n- [Guide](/guide.md): the guide\n- [Old](https://example.com/old.md)\n",
    );
    respond(
        "https://example.com/guide.md",
        200,
        "text/markdown",
        "# Guide",
    );
    respond("https://example.com/old.md", 404, "text/plain", "");
    respond(
        "https://example.com/index.html.md",
        200,
        "text/markdown",
        "# Example",
    );

    let access = tools::ai_access("https://example.com/".into()).unwrap();
    let rule = |agent: &str| {
        let crawler = access.crawlers.iter().find(|c| c.agent == agent).unwrap();
        (&crawler.rule, crawler.group.as_deref())
    };
    assert_eq!(rule("GPTBot"), (&Rule::Blocked, Some("GPTBot")));
    assert_eq!(rule("ClaudeBot"), (&Rule::Allowed, Some("ClaudeBot")));
    assert_eq!(rule("CCBot"), (&Rule::Allowed, Some("*")));
    assert_eq!(access.llms.title.as_deref(), Some("Example"));
    assert_eq!(access.llms.links, 2);
    assert_eq!(access.llms.broken, ["https://example.com/old.md"]);
    assert!(access.markdown_suffix);
    assert!(!access.markdown_accept);
    let codes: Vec<_> = access
        .findings
        .iter()
        .map(|finding| &finding.code)
        .collect();
    assert_eq!(codes, [&Code::BlocksAiCrawler, &Code::BrokenLlmsTxtLink]);
    assert_eq!(access.page.title.as_deref(), Some("Example"));
}

#[test]
fn an_app_shell_without_robots_or_llms_txt_is_reported() {
    respond(
        "https://example.com/",
        200,
        "text/html",
        r#"<html><head><title>App</title></head><body><div id="root"></div><script src="/app.js"></script><script type="application/ld+json">{}</script></body></html>"#,
    );
    respond("https://example.com/robots.txt", 404, "text/html", "");
    respond("https://example.com/llms.txt", 404, "text/html", "");
    respond("https://example.com/index.html.md", 404, "text/html", "");

    let access = tools::ai_access("https://example.com/".into()).unwrap();
    assert_eq!(access.robots_status, Some(404));
    assert!(
        access
            .crawlers
            .iter()
            .all(|crawler| crawler.rule == Rule::Unspecified)
    );
    assert_eq!(access.scripts, 1);
    let codes: Vec<_> = access
        .findings
        .iter()
        .map(|finding| &finding.code)
        .collect();
    assert_eq!(
        codes,
        [
            &Code::MissingLlmsTxt,
            &Code::ContentNeedsJavaScript,
            &Code::NoMarkdownCopy
        ]
    );
}

#[test]
fn the_longest_pattern_wins_and_wildcards_and_anchors_match() {
    let robots = Robots::from(
        "User-agent: *\nDisallow: /*.pdf$\nAllow: /docs\nDisallow: /docs/private # staff\n",
    );
    let rule = |path| robots.crawler("GPTBot", path).rule;
    assert_eq!(rule("/a/b.pdf"), Rule::Blocked);
    assert_eq!(rule("/a/b.pdf?page=2"), Rule::Allowed);
    assert_eq!(rule("/docs/private/x"), Rule::Blocked);
    assert_eq!(rule("/docs/public"), Rule::Allowed);
    assert_eq!(rule("/"), Rule::Allowed);
}
