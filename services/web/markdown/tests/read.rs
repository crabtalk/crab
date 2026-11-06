use berm_lang::{http::Response, test};
use webpage::{
    page::{MAX_ATTRIBUTES, MAX_DEPTH, MAX_HTML, MAX_NODES, MAX_TAGS, MAX_TOTAL_ATTRIBUTES, Page},
    tools,
};

fn response(status: u16, headers: &[(&str, &str)], body: &[u8]) -> Response {
    Response {
        status,
        headers: headers
            .iter()
            .map(|(key, value)| (key.to_string(), value.to_string()))
            .collect(),
        body: body.to_vec(),
    }
}

#[test]
fn extracts_main_content_preserving_structure_and_resolving_links() {
    test::respond_with("GET", "https://example.com/docs/start", response(200, &[("Content-Type", "text/html")], br#"
        <html><head><title>Read &amp; use</title><base href="/guide/"><meta name="description" content="A guide"></head>
        <body><nav>Outside navigation</nav><main><h1>Start</h1><nav>Inside navigation</nav>
        <p>Hello <strong>reader</strong>. <a href="next">Continue</a></p>
        <ul><li>One</li><li>Two</li></ul><pre><code class="language-rust">let x = 1;</code></pre>
        <script>alert("ignored")</script><footer>Footer</footer></main><aside>Sidebar</aside></body></html>"#));
    let read = tools::read("https://example.com/docs/start".into()).unwrap();
    assert_eq!(read.title.as_deref(), Some("Read & use"));
    assert_eq!(read.description.as_deref(), Some("A guide"));
    assert_eq!(read.region, "main");
    assert!(read.markdown.contains("# Start"));
    assert!(read.markdown.contains("**reader**"));
    assert!(read.markdown.contains("https://example.com/guide/next"));
    assert!(
        read.markdown.lines().any(|line| {
            line.trim_start()
                .strip_prefix('-')
                .or_else(|| line.trim_start().strip_prefix('*'))
                .is_some_and(|item| item.trim() == "One")
        }),
        "{}",
        read.markdown
    );
    assert!(read.markdown.contains("let x = 1;"));
    for omitted in [
        "Outside navigation",
        "Inside navigation",
        "Sidebar",
        "alert",
        "Footer",
    ] {
        assert!(!read.markdown.contains(omitted), "{omitted}");
    }
}

#[test]
fn follows_relative_redirects_and_uses_the_final_page_url() {
    test::respond_with(
        "GET",
        "https://example.com/old",
        response(302, &[("location", "/new")], b""),
    );
    test::respond_with(
        "GET",
        "https://example.com/new",
        response(
            200,
            &[("content-type", "text/html; charset=utf-8")],
            b"<article><a href='next'>Next</a></article>",
        ),
    );
    let read = tools::read("https://example.com/old#fragment".into()).unwrap();
    assert_eq!(read.url, "https://example.com/new");
    assert!(read.markdown.contains("https://example.com/next"));
}

#[test]
fn refuses_unsupported_urls_redirect_loops_and_non_html_responses() {
    assert!(Page::fetch("file:///etc/passwd").is_err());
    assert!(Page::fetch("https://user:password@example.com").is_err());
    test::respond_with(
        "GET",
        "https://example.com/loop",
        response(301, &[("location", "/loop")], b""),
    );
    assert!(
        Page::fetch("https://example.com/loop")
            .err()
            .unwrap()
            .message
            .contains("loop")
    );
    test::respond_with(
        "GET",
        "https://example.com/json",
        response(200, &[("content-type", "application/json")], b"{}"),
    );
    assert!(Page::fetch("https://example.com/json").is_err());
    test::respond_with(
        "GET",
        "https://example.com/fail",
        response(503, &[("content-type", "text/html")], b"Unavailable"),
    );
    assert!(Page::fetch("https://example.com/fail").is_err());
}

#[test]
fn refuses_oversized_html_and_empty_extraction() {
    test::respond_with(
        "GET",
        "https://example.com/large",
        response(
            200,
            &[("content-type", "text/html")],
            &vec![b'a'; MAX_HTML + 1],
        ),
    );
    assert!(Page::fetch("https://example.com/large").is_err());
    test::respond_with(
        "GET",
        "https://example.com/empty",
        response(
            200,
            &[("content-type", "text/html")],
            b"<body><script>onlyJS()</script></body>",
        ),
    );
    assert!(tools::read("https://example.com/empty".into()).is_err());
}

#[test]
fn bounds_html_nodes_and_depth_during_parsing() {
    for (body, expected) in [
        ("<p>x</p>".repeat(MAX_NODES), "HTML node limit"),
        (
            format!(
                "{}x{}",
                "<div>".repeat(MAX_DEPTH + 1),
                "</div>".repeat(MAX_DEPTH + 1)
            ),
            "HTML nesting limit",
        ),
    ] {
        test::respond_with(
            "GET",
            "https://example.com/complex",
            response(200, &[("content-type", "text/html")], body.as_bytes()),
        );
        assert!(
            Page::fetch("https://example.com/complex")
                .err()
                .unwrap()
                .message
                .contains(expected)
        );
    }
}

#[test]
fn incremental_parsing_preserves_unicode_across_chunk_boundaries() {
    let text = "A🙂é".repeat(500);
    test::respond_with(
        "GET",
        "https://example.com/unicode",
        response(
            200,
            &[("content-type", "text/html")],
            format!("<main><p>{text}</p></main>").as_bytes(),
        ),
    );
    let read = tools::read("https://example.com/unicode".into()).unwrap();
    assert_eq!(read.markdown.trim(), text);
}

#[test]
fn bounds_attributes_before_dom_tokenization() {
    let attributes = (0..MAX_ATTRIBUTES + 1)
        .map(|i| format!(" a{i}='x'"))
        .collect::<String>();
    test::respond_with(
        "GET",
        "https://example.com/attributes",
        response(
            200,
            &[("content-type", "text/html")],
            format!("<main{attributes}>text</main>").as_bytes(),
        ),
    );
    assert!(
        Page::fetch("https://example.com/attributes")
            .err()
            .unwrap()
            .message
            .contains("HTML attribute limit")
    );
}

#[test]
fn bounds_total_token_work_even_when_the_dom_would_be_small() {
    for (body, expected) in [
        ("<html>".repeat(MAX_TAGS + 1), "HTML tag limit"),
        (
            format!(
                "<main>{}</main>",
                "<p a='x' b='x'>text</p>".repeat(MAX_TOTAL_ATTRIBUTES / 2 + 1)
            ),
            "total HTML attribute limit",
        ),
    ] {
        test::respond_with(
            "GET",
            "https://example.com/tokens",
            response(200, &[("content-type", "text/html")], body.as_bytes()),
        );
        assert!(
            Page::fetch("https://example.com/tokens")
                .err()
                .unwrap()
                .message
                .contains(expected)
        );
    }
}

#[test]
fn svg_titles_do_not_supply_the_page_title() {
    let page = Page::try_from((
        "https://example.com/".parse().unwrap(),
        response(
            200,
            &[("content-type", "text/html")],
            b"<body><svg><title>Icon label</title></svg><main>Content</main></body>",
        ),
    ))
    .unwrap();
    assert_eq!(page.title(), None);
}
