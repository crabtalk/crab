use berm_lang::{http::Response, test};
use seo::{
    access::{Access, Code},
    crawlers::{AGENTS, Robots, Rule},
    report::Level,
    tools,
};
use webpage::page::Page;

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
    test::respond_with(
        "GET",
        "https://example.com/",
        Response {
            status: 200,
            headers: vec![
                ("content-type".into(), "text/html".into()),
                ("Link".into(), r#"</catalog>; rel="api-catalog", </api>; rel="service-desc service-doc"; title="API, docs""#.into()),
                ("link".into(), r#"</guide.md>; rel=alternate; type="Text/Markdown; charset=utf-8", </guide.html>; rel=alternate; type="text/html""#.into()),
                ("LINK".into(), r#"</llms.txt>; rel=describedby, </>; rel=canonical"#.into()),
            ],
            body: format!(
                r#"<html lang="en"><head><base href="/another/"><title>Example</title></head><body><h1>Example</h1><p>{words}</p></body></html>"#
            ).into_bytes(),
        },
    );
    respond(
        "https://example.com/robots.txt",
        200,
        "text/plain",
        "User-agent: GPTBot\nDisallow: /\nContent-Signal: ai-input=no, search=no\n\nUser-agent: ClaudeBot\nAllow: /\n\nUser-agent: *\nDisallow: /private\nContent-Signal: ai-train=no, search=yes, ai-input=yes\n",
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
    assert_eq!(
        codes,
        [
            &Code::BlocksAiCrawler,
            &Code::RestrictiveContentSignal,
            &Code::RestrictiveContentSignal,
            &Code::BrokenLlmsTxtLink,
        ]
    );
    assert!(
        access.findings[1..3].iter().all(|finding| {
            finding.level == Level::Warning && finding.message.contains("GPTBot")
        })
    );
    let signals = |agent: &str| {
        &access
            .crawlers
            .iter()
            .find(|c| c.agent == agent)
            .unwrap()
            .content_signals
    };
    assert_eq!(signals("ClaudeBot").search, None);
    assert_eq!(signals("CCBot").ai_train, Some(false));
    assert_eq!(signals("CCBot").ai_input, Some(true));
    let links: Vec<_> = access
        .agent_links
        .iter()
        .map(|link| link.rel.as_str())
        .collect();
    assert_eq!(
        links,
        [
            "api-catalog",
            "service-desc",
            "service-doc",
            "alternate",
            "describedby"
        ]
    );
    assert_eq!(access.agent_links[3].target, "https://example.com/guide.md");
    assert_eq!(access.agent_links[3].context, "https://example.com/");
    assert_eq!(
        access.agent_links[3].media_type.as_deref(),
        Some("Text/Markdown; charset=utf-8")
    );
    assert_eq!(
        test::fetched()
            .iter()
            .filter(|request| request.url == "https://example.com/")
            .count(),
        2
    );
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
    assert!(access.agent_links.is_empty());
    assert_eq!(access.findings.last().unwrap().level, Level::Info);
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
            &Code::NoMarkdownCopy,
            &Code::MissingAgentLinks,
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

#[test]
fn combines_signals_only_from_applicable_groups() {
    let robots = Robots::from(concat!(
        "Content-Signal: ai-input=no\n",
        "User-agent: ClaudeBot\nUser-agent: GPTBot\n",
        "Content-Signal: Search=yes, ai-input=invalid, unknown=no\nAllow: /\n",
        "User-agent: *\nContent-Signal: ai-input=no, ai-train=yes\nAllow: /\n",
        "User-agent: gptbot\nContent-Signal: search = NO, ai-train=no # comment\n",
    ));
    let gpt = robots.crawler("GPTBot", "/");
    assert_eq!(gpt.group.as_deref(), Some("GPTBot"));
    assert_eq!(gpt.content_signals.search, Some(false));
    assert_eq!(gpt.content_signals.ai_input, None);
    assert_eq!(gpt.content_signals.ai_train, Some(false));
    let claude = robots.crawler("ClaudeBot", "/");
    assert_eq!(claude.content_signals.search, Some(true));
    assert_eq!(claude.content_signals.ai_train, None);
    let fallback = robots.crawler("CCBot", "/");
    assert_eq!(fallback.group.as_deref(), Some("*"));
    assert_eq!(fallback.content_signals.ai_input, Some(false));
}

#[test]
fn content_signal_between_user_agents_preserves_the_group() {
    let robots =
        Robots::from("User-agent: A\nContent-Signal: ai-input=no\nUser-agent: B\nDisallow: /\n");
    for agent in ["A", "B"] {
        let crawler = robots.crawler(agent, "/");
        assert_eq!(crawler.rule, Rule::Blocked);
        assert_eq!(crawler.content_signals.ai_input, Some(false));
    }
}

#[test]
fn warns_once_per_robots_group_and_signal() {
    respond("https://example.com/", 200, "text/html", "<h1>Example</h1>");
    respond("https://example.com/llms.txt", 404, "text/plain", "");
    respond("https://example.com/index.html.md", 404, "text/plain", "");
    for (robots, affected) in [
        (
            "User-agent: *\nContent-Signal: ai-input=no, search=no\n",
            AGENTS.as_slice(),
        ),
        (
            "User-agent: GPTBot\nUser-agent: ClaudeBot\nContent-Signal: ai-input=no, search=no\n",
            &["GPTBot", "ClaudeBot"][..],
        ),
    ] {
        respond("https://example.com/robots.txt", 200, "text/plain", robots);
        let access = tools::ai_access("https://example.com/".into()).unwrap();
        let warnings: Vec<_> = access
            .findings
            .iter()
            .filter(|finding| finding.code == Code::RestrictiveContentSignal)
            .collect();
        assert_eq!(warnings.len(), 2);
        for (warning, signal) in warnings.into_iter().zip(["ai-input=no", "search=no"]) {
            assert_eq!(warning.level, Level::Warning);
            assert!(warning.message.contains(signal));
            assert!(warning.message.contains(&affected.join(", ")));
        }
        for crawler in access
            .crawlers
            .iter()
            .filter(|crawler| affected.contains(&crawler.agent.as_str()))
        {
            assert_eq!(crawler.content_signals.ai_input, Some(false));
            assert_eq!(crawler.content_signals.search, Some(false));
        }
    }
}

#[test]
fn markdown_suffix_requires_supported_media_type_and_body() {
    respond("https://example.com/", 200, "text/html", "<h1>Example</h1>");
    respond("https://example.com/robots.txt", 404, "text/plain", "");
    respond("https://example.com/llms.txt", 404, "text/plain", "");
    for (kind, body, expected) in [
        ("application/json", r#"{"error":"not found"}"#, false),
        ("text/markdown", "", false),
        ("text/html", "<h1>Not found</h1>", false),
        ("text/markdown", "# Example", true),
        ("Text/X-Markdown; charset=UTF-8", "# Example", true),
        ("text/plain; charset=utf-8", "# Example", true),
    ] {
        respond("https://example.com/index.html.md", 200, kind, body);
        let access = tools::ai_access("https://example.com/".into()).unwrap();
        assert_eq!(access.markdown_suffix, expected, "{kind}, {body:?}");
        assert_eq!(
            access
                .findings
                .iter()
                .any(|finding| finding.code == Code::NoMarkdownCopy),
            !expected
        );
    }
}

#[test]
fn negotiated_markdown_requires_vary_accept() {
    let url = "https://example.com/";
    let page = Page::try_from((
        url.parse().unwrap(),
        Response {
            status: 200,
            headers: vec![("Content-Type".into(), "text/html".into())],
            body: b"<html><body><h1>Example</h1></body></html>".to_vec(),
        },
    ))
    .unwrap();
    respond("https://example.com/robots.txt", 404, "text/html", "");
    respond("https://example.com/llms.txt", 404, "text/html", "");
    respond("https://example.com/index.html.md", 404, "text/html", "");

    for vary in [
        None,
        Some("Accept-Language"),
        Some("Accept-Encoding, aCcEpT"),
    ] {
        let mut headers = vec![("Content-Type".into(), "Text/Markdown; charset=utf-8".into())];
        if let Some(value) = vary {
            headers.push(("Vary".into(), "User-Agent".into()));
            headers.push(("vArY".into(), value.into()));
        }
        test::respond_with(
            "GET",
            url,
            Response {
                status: 200,
                headers,
                body: b"# Example".to_vec(),
            },
        );
        let access = Access::try_from(&page).unwrap();
        assert!(access.markdown_accept);
        assert!(
            !access
                .findings
                .iter()
                .any(|finding| finding.code == Code::NoMarkdownCopy)
        );
        let warning = access
            .findings
            .iter()
            .find(|finding| finding.code == Code::MarkdownMissingVaryAccept);
        assert_eq!(
            warning.map(|finding| &finding.level),
            if vary.is_some_and(|vary| vary.contains(',')) {
                None
            } else {
                Some(&Level::Warning)
            }
        );
    }
    assert!(
        test::fetched()
            .iter()
            .filter(|request| request.url == url)
            .all(|request| request.header("accept") == Some("text/markdown"))
    );
}
