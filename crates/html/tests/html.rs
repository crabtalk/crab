#[test]
fn text_ends_a_tag_outside_quotes() {
    assert_eq!(
        crab_html::text(r#"<a title="x>y" href="/z">Go</a>"#),
        "[Go](/z)"
    );
}

#[test]
fn link_text_keeps_tags_sharing_its_prefix() {
    let links = crab_html::links(
        r#"<a href="/g"><abbr>HTML</abbr> guide</a><aside>no</aside><a href="/h">Next <article>x</article></a>"#,
    );
    assert_eq!(links.len(), 2);
    assert_eq!(links[0].text, "HTML guide");
    assert_eq!(links[1].text, "Next x");
}

#[test]
fn link_without_close_runs_to_the_next_link() {
    let links = crab_html::links(r#"<a href="/a">A <a href="/b">B</a>"#);
    assert_eq!(links[0].text, "A");
    assert_eq!(links[1].text, "B");
}

#[test]
fn element_ends_at_its_own_closing_tag() {
    let page = "<pre>skip</pre><p>Code <pre>x</pre> and <path></path>more</p><p>no</p>";
    assert_eq!(
        crab_html::element(page, "p", None).as_deref(),
        Some("Code x and more")
    );
}

#[test]
fn element_skips_a_longer_tag_name() {
    let page = r#"<pre class="c">no</pre><p class="c">yes</p>"#;
    assert_eq!(
        crab_html::element(page, "p", Some("c")).as_deref(),
        Some("yes")
    );
}

#[test]
fn unknown_entity_is_kept_as_written() {
    assert_eq!(crab_html::unescape("?a=1&utm=x;&amp;b"), "?a=1&utm=x;&b");
    assert_eq!(crab_html::text("x &bogus; y"), "x &bogus; y");
}

#[test]
fn head_answers_what_title_and_meta_answer() {
    let page = concat!(
        r#"<META NAME="description" content="  "><Title>A &amp; B</TITLE>"#,
        r#"<meta content="<meta name='og:title' content='inner'>" property="og:title">"#,
        r#"<body><meta name="Description" content="Late"></body>"#,
    );
    let properties = ["og:title", "description", "og:image"];
    let head = crab_html::head(page, &properties);
    assert_eq!(head.title, crab_html::title(page));
    assert_eq!(head.title.as_deref(), Some("A & B"));
    let metas: Vec<Option<String>> = properties
        .iter()
        .map(|property| crab_html::meta(page, property))
        .collect();
    assert_eq!(head.metas, metas);
    assert_eq!(
        head.metas,
        [
            Some(String::from("<meta name='og:title' content='inner'>")),
            Some(String::from("Late")),
            None,
        ]
    );
}
