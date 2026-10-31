//! Text out of HTML: the fields feeds and APIs carry, and the `<meta>`,
//! links, scripts and elements of a page.

#![no_std]

extern crate alloc;

use alloc::{string::String, vec::Vec};

/// The longest entity body decoded, `#1114111`, in bytes.
const LONGEST_ENTITY: usize = 8;

/// `html` as text. Tags are dropped, and `<script>` and `<style>` with their
/// contents, except that a block boundary becomes a line break and a link
/// becomes `[text](href)`, or its text alone when that is its href or it has
/// none. Entities are decoded; an unknown named one, and an `&` with no `;`
/// within 8 bytes, are kept as written. An unclosed tag, quoted values
/// considered, drops the rest.
///
/// ```
/// let said = crab_html::text("It&#x27;s <i>fine</i>.<p>Next");
/// assert_eq!(said, "It's fine.\n\nNext");
/// ```
pub fn text(html: &str) -> String {
    let bytes = html.as_bytes();
    let mut out = String::with_capacity(html.len());
    let mut link = None;
    let mut at = 0;
    while let Some(found) = bytes[at..].iter().position(|&b| b == b'<' || b == b'&') {
        let mark = at + found;
        out.push_str(&html[at..mark]);
        if bytes[mark] == b'&' {
            at = entity(&mut out, html, mark);
            continue;
        }
        let end = tag_end(&html[mark..]);
        if mark + end == html.len() {
            return finish(out);
        }
        let tag = &html[mark + 1..mark + end];
        at = mark + end + 1;

        let (closing, tag) = match tag.strip_prefix('/') {
            Some(tag) => (true, tag),
            None => (false, tag),
        };
        let name = tag
            .split(|c: char| c.is_ascii_whitespace() || c == '/')
            .next()
            .unwrap_or("");
        let is = |candidate: &str| name.eq_ignore_ascii_case(candidate);
        if !closing && (is("script") || is("style")) {
            let close = if is("script") { "</script" } else { "</style" };
            at = match find_ignore_case(&html[at..], close) {
                Some(found) => at + found + tag_end(&html[at + found..]) + 1,
                None => html.len(),
            }
            .min(html.len());
            continue;
        }
        if is("a") {
            match closing {
                false => {
                    link = Some((out.len(), attribute(tag, "href")));
                    out.push('[');
                }
                true => {
                    let Some((start, href)) = link.take() else {
                        continue;
                    };
                    let mut target = String::new();
                    if let Some(href) = href {
                        decode(&mut target, href);
                    }
                    if target.is_empty() || out[start + 1..] == target {
                        out.remove(start);
                    } else {
                        out.push_str("](");
                        out.push_str(&target);
                        out.push(')');
                    }
                }
            }
        } else if is("br") || is("li") || is("tr") {
            line_break(&mut out, 1);
        } else if ["p", "div", "pre", "blockquote", "ul", "ol", "table"]
            .iter()
            .any(|block| is(block))
            || (name.len() == 2
                && name.as_bytes()[0].eq_ignore_ascii_case(&b'h')
                && name.as_bytes()[1].is_ascii_digit())
        {
            line_break(&mut out, 2);
        }
    }
    out.push_str(&html[at..]);
    finish(out)
}

/// `s` with its entities decoded, the same entities [`text`] decodes, and
/// nothing else changed.
pub fn unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    decode(&mut out, s);
    out
}

/// The text of the first `<title>`: entities decoded, whitespace collapsed to
/// single spaces. `None` when there is none or it is blank.
///
/// ```
/// assert_eq!(crab_html::title("<title> Fish &amp;\n chips </title>").as_deref(), Some("Fish & chips"));
/// ```
pub fn title(html: &str) -> Option<String> {
    scan(html, true, &[]).title
}

/// The `content` of the first `<meta>` whose `property` or `name` is
/// `property`, ASCII case ignored, with a non-blank content: entities decoded,
/// whitespace collapsed to single spaces.
///
/// ```
/// let page = r#"<meta property="og:title" content="Fish &amp; chips">"#;
/// assert_eq!(crab_html::meta(page, "og:title").as_deref(), Some("Fish & chips"));
/// ```
pub fn meta(html: &str, property: &str) -> Option<String> {
    scan(html, false, &[property]).metas.pop().flatten()
}

/// What [`title`] and [`meta`] answer of a page, read in one pass.
pub struct Head {
    pub title: Option<String>,
    /// What [`meta`] answers for each property asked, in the order asked.
    pub metas: Vec<Option<String>>,
}

/// [`title`] and [`meta`] for each of `properties` of `html`, read in one pass
/// that ends once each is found.
///
/// ```
/// let page = r#"<title>Fish</title><meta name="description" content="Chips">"#;
/// let head = crab_html::head(page, &["og:title", "description"]);
/// assert_eq!(head.title.as_deref(), Some("Fish"));
/// assert_eq!(head.metas, [None, Some(String::from("Chips"))]);
/// ```
pub fn head(html: &str, properties: &[&str]) -> Head {
    scan(html, true, properties)
}

/// [`head`], reading the title only when `titled`.
fn scan(html: &str, titled: bool, properties: &[&str]) -> Head {
    let bytes = html.as_bytes();
    let mut head = Head {
        title: None,
        metas: properties.iter().map(|_| None).collect(),
    };
    let mut titled = titled;
    let mut wanted = properties.len();
    // Where the next `<meta` may start: past the last one's tag.
    let mut metas_from = 0;
    let mut at = 0;
    while titled || wanted > 0 {
        let Some(found) = bytes[at..].iter().position(|&b| b == b'<') else {
            break;
        };
        let mark = at + found;
        at = mark + 1;
        let rest = &bytes[mark..];
        if titled && starts_ignore_case(rest, "<title") {
            titled = false;
            let start = mark + tag_end(&html[mark..]) + 1;
            head.title = html.get(start..).and_then(|inner| {
                let end = start + find_ignore_case(inner, "</title")?;
                collapse(&unescape(&html[start..end]))
            });
        }
        if wanted == 0 || mark < metas_from || !starts_ignore_case(rest, "<meta") {
            continue;
        }
        let start = mark + "<meta".len();
        let end = start + tag_end(&html[start..]);
        metas_from = end;
        let tag = &html[start..end];
        let mut content = None;
        for (property, found) in properties.iter().zip(&mut head.metas) {
            let named = found.is_none()
                && ["property", "name"].iter().any(|key| {
                    attribute(tag, key).is_some_and(|value| value.eq_ignore_ascii_case(property))
                });
            if !named {
                continue;
            }
            let content = content.get_or_insert_with(|| {
                collapse(&unescape(attribute(tag, "content").unwrap_or_default()))
            });
            if content.is_some() {
                *found = content.clone();
                wanted -= 1;
            }
        }
    }
    head
}

/// A link on a page: its `href` with entities decoded, and its text.
pub struct Link {
    pub href: String,
    /// What [`text`] makes of its contents, whitespace collapsed.
    pub text: String,
    /// The `alt` of the first `<img>` in it, entities decoded and whitespace
    /// collapsed; `None` when it has none or it is blank.
    pub image: Option<String>,
}

/// Each `<a>` with an `href`, in page order. A link whose `</a>` is missing
/// runs to the next `<a>`.
///
/// ```
/// let links = crab_html::links(r#"<a href="/a?x=1&amp;y=2">A <b>b</b></a><a href="/c"><img src="c.png" alt="C &amp; D"></a>"#);
/// assert_eq!((links[0].href.as_str(), links[0].text.as_str()), ("/a?x=1&y=2", "A b"));
/// assert_eq!(links[1].image.as_deref(), Some("C & D"));
/// ```
pub fn links(html: &str) -> Vec<Link> {
    let mut links = Vec::new();
    let mut at = 0;
    while let Some((found, closing)) = find_tag(&html[at..], "a") {
        let start = at + found + "<a".len();
        at = start;
        if closing {
            continue;
        }
        let open = start + tag_end(&html[start..]);
        let Some(href) = attribute(&html[start..open], "href") else {
            continue;
        };
        let inner = (open + 1).min(html.len());
        let close = find_tag(&html[inner..], "a").map_or(html.len(), |(end, _)| inner + end);
        let contents = &html[inner..close];
        let image = find_ignore_case(contents, "<img").and_then(|found| {
            let tag = &contents[found + "<img".len()..];
            collapse(&unescape(attribute(&tag[..tag_end(tag)], "alt")?))
        });
        links.push(Link {
            href: unescape(href),
            text: collapse(&text(contents)).unwrap_or_default(),
            image,
        });
        at = close;
    }
    links
}

/// The contents of each `<script>` whose `type` is `kind`, ASCII case
/// ignored, as written: `application/ld+json` for a page's JSON-LD.
pub fn scripts<'a>(html: &'a str, kind: &str) -> Vec<&'a str> {
    let mut scripts = Vec::new();
    let mut at = 0;
    while let Some(found) = find_ignore_case(&html[at..], "<script") {
        let start = at + found + "<script".len();
        let open = start + tag_end(&html[start..]);
        let inner = (open + 1).min(html.len());
        let close =
            find_ignore_case(&html[inner..], "</script").map_or(html.len(), |end| inner + end);
        at = close;
        if attribute(&html[start..open], "type")
            .is_some_and(|value| value.trim().eq_ignore_ascii_case(kind))
        {
            scripts.push(&html[inner..close]);
        }
    }
    scripts
}

/// What [`text`] makes of the first element named `name`, an ASCII lowercase
/// tag name, of `class` when given, one of the words of its `class`;
/// whitespace collapsed to single spaces. The element ends at the first
/// closing tag of its name. `None` when there is none or it is blank.
///
/// ```
/// let page = r#"<p>Not this</p><p class="tag line">Ada <i>L.</i></p>"#;
/// assert_eq!(crab_html::element(page, "p", Some("tag")).as_deref(), Some("Ada L."));
/// ```
pub fn element(html: &str, name: &str, class: Option<&str>) -> Option<String> {
    let mut at = 0;
    while let Some((found, closing)) = find_tag(&html[at..], name) {
        let start = at + found + 1 + name.len();
        at = start;
        if closing {
            continue;
        }
        let end = start + tag_end(&html[start..]);
        let classed = class.is_none_or(|class| {
            attribute(&html[start..end], "class")
                .is_some_and(|classes| classes.split_ascii_whitespace().any(|word| word == class))
        });
        if !classed {
            continue;
        }
        let inner = (end + 1).min(html.len());
        let mut finish = inner;
        let finish = loop {
            match find_tag(&html[finish..], name) {
                Some((found, true)) => break finish + found,
                Some((found, false)) => finish += found + 1,
                None => break html.len(),
            }
        };
        return collapse(&text(&html[inner..finish]));
    }
    None
}

/// The offset of the `>` that ends the tag `s` is inside, outside quoted
/// values; `s.len()` when there is none.
fn tag_end(s: &str) -> usize {
    let mut quote = None;
    for (i, c) in s.char_indices() {
        match (quote, c) {
            (None, '"' | '\'') => quote = Some(c),
            (Some(q), c) if c == q => quote = None,
            (None, '>') => return i,
            _ => {}
        }
    }
    s.len()
}

/// The offset of the first match of `needle`, an ASCII lowercase string, in
/// `s`, ASCII case ignored.
fn find_ignore_case(s: &str, needle: &str) -> Option<usize> {
    let bytes = s.as_bytes();
    let first = *needle.as_bytes().first()?;
    let mut at = 0;
    while let Some(found) = bytes[at..]
        .iter()
        .position(|b| b.eq_ignore_ascii_case(&first))
    {
        let mark = at + found;
        if starts_ignore_case(&bytes[mark..], needle) {
            return Some(mark);
        }
        at = mark + 1;
    }
    None
}

/// Whether `bytes` starts with `prefix`, an ASCII lowercase string, ASCII case
/// ignored.
fn starts_ignore_case(bytes: &[u8], prefix: &str) -> bool {
    bytes
        .get(..prefix.len())
        .is_some_and(|start| start.eq_ignore_ascii_case(prefix.as_bytes()))
}

/// The offset of the first `<name` or `</name` in `s`, `name` an ASCII
/// lowercase tag name, ASCII case ignored, followed by whitespace, `>`, `/` or
/// the end of `s`; and whether it is the closing one.
fn find_tag(s: &str, name: &str) -> Option<(usize, bool)> {
    let bytes = s.as_bytes();
    let mut at = 0;
    while let Some(found) = bytes[at..].iter().position(|&b| b == b'<') {
        let mark = at + found;
        at = mark + 1;
        let closing = bytes.get(at) == Some(&b'/');
        let start = at + usize::from(closing);
        let candidate = bytes.get(start..start + name.len())?;
        let bounded = bytes
            .get(start + name.len())
            .is_none_or(|&b| b.is_ascii_whitespace() || b == b'>' || b == b'/');
        if bounded && candidate.eq_ignore_ascii_case(name.as_bytes()) {
            return Some((mark, closing));
        }
    }
    None
}

/// `s` with its whitespace collapsed to single spaces and trimmed; `None` when
/// it is blank.
fn collapse(s: &str) -> Option<String> {
    let mut out = String::new();
    for word in s.split_whitespace() {
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(word);
    }
    (!out.is_empty()).then_some(out)
}

fn finish(mut out: String) -> String {
    out.truncate(out.trim_end().len());
    out
}

/// End `out` with `newlines` line breaks, dropping the spaces before them,
/// unless it is empty or already ends with as many.
fn line_break(out: &mut String, newlines: usize) {
    out.truncate(out.trim_end_matches([' ', '\t']).len());
    if out.is_empty() {
        return;
    }
    let have = out.bytes().rev().take_while(|&b| b == b'\n').count();
    for _ in have..newlines {
        out.push('\n');
    }
}

/// The value of `name` in a tag's attributes, quoted with `"` or `'`, or bare
/// up to whitespace.
fn attribute<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let bytes = tag.as_bytes();
    let mut from = 0;
    while let Some(found) = bytes[from..].iter().position(|&b| b == b'=') {
        let equals = from + found;
        let key = tag[..equals].trim_end();
        let key = &key[key
            .rfind(|c: char| c.is_ascii_whitespace())
            .map_or(0, |i| i + 1)..];
        let value = tag[equals + 1..].trim_start();
        let (value, rest) = match value.chars().next()? {
            quote @ ('"' | '\'') => {
                let close = value[1..].find(quote)?;
                (&value[1..1 + close], 1 + close + 1)
            }
            _ => {
                let end = value
                    .find(|c: char| c.is_ascii_whitespace())
                    .unwrap_or(value.len());
                (&value[..end], end)
            }
        };
        if key.eq_ignore_ascii_case(name) {
            return Some(value);
        }
        from = tag.len() - tag[equals + 1..].trim_start().len() + rest;
    }
    None
}

/// Append `run` to `out`, entities decoded.
fn decode(out: &mut String, run: &str) {
    let bytes = run.as_bytes();
    let mut at = 0;
    while let Some(found) = bytes[at..].iter().position(|&b| b == b'&') {
        let mark = at + found;
        out.push_str(&run[at..mark]);
        at = entity(out, run, mark);
    }
    out.push_str(&run[at..]);
}

/// Append the entity at `mark`, the index of an `&` in `s`, and answer the
/// index after it.
fn entity(out: &mut String, s: &str, mark: usize) -> usize {
    match s.as_bytes()[mark + 1..]
        .iter()
        .take(LONGEST_ENTITY + 1)
        .position(|&b| b == b';')
    {
        Some(end) => {
            push_entity(out, &s[mark + 1..mark + 1 + end]);
            mark + end + 2
        }
        None => {
            out.push('&');
            mark + 1
        }
    }
}

/// Decode one entity body, the text between `&` and `;`. An unknown name is
/// kept as written, `&` and `;` included; an invalid number is dropped.
fn push_entity(out: &mut String, name: &str) {
    if let Some(num) = name.strip_prefix('#') {
        let code = match num.strip_prefix(['x', 'X']) {
            Some(hex) => u32::from_str_radix(hex, 16).ok(),
            None => num.parse().ok(),
        };
        if let Some(ch) = code.and_then(char::from_u32) {
            out.push(ch);
        }
        return;
    }
    let known = match name {
        "amp" => "&",
        "lt" => "<",
        "gt" => ">",
        "quot" => "\"",
        "apos" => "'",
        "nbsp" => " ",
        _ => {
            out.push('&');
            out.push_str(name);
            out.push(';');
            return;
        }
    };
    out.push_str(known);
}
