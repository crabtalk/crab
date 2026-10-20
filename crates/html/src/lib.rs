//! Text out of HTML: the fields feeds and APIs carry, and the `<meta>` of a
//! page.

#![no_std]

extern crate alloc;

use alloc::{string::String, vec::Vec};

/// The longest entity body decoded, `#1114111`, in bytes.
const LONGEST_ENTITY: usize = 8;

/// `html` as text. Tags are dropped, except that a block boundary becomes a
/// line break and a link becomes `[text](href)`, or its text alone when that
/// is its href or it has none. Entities are decoded; an `&` with no `;` within
/// 8 bytes is kept. An unclosed tag drops the rest.
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
        let Some(end) = bytes[mark..].iter().position(|&b| b == b'>') else {
            return finish(out);
        };
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

/// The `content` of the first `<meta>` whose `property` or `name` is
/// `property`, ASCII case ignored, with a non-blank content: entities decoded,
/// whitespace collapsed to single spaces.
///
/// ```
/// let page = r#"<meta property="og:title" content="Fish &amp; chips">"#;
/// assert_eq!(crab_html::meta(page, "og:title").as_deref(), Some("Fish & chips"));
/// ```
pub fn meta(html: &str, property: &str) -> Option<String> {
    let mut at = 0;
    while let Some(found) = find_ignore_case(&html[at..], "<meta") {
        let start = at + found + "<meta".len();
        let end = start + tag_end(&html[start..]);
        at = end;
        let tag = &html[start..end];
        let named = ["property", "name"].iter().any(|key| {
            attribute(tag, key).is_some_and(|value| value.eq_ignore_ascii_case(property))
        });
        if !named {
            continue;
        }
        let content = unescape(attribute(tag, "content").unwrap_or_default());
        let words: Vec<&str> = content.split_whitespace().collect();
        if !words.is_empty() {
            return Some(words.join(" "));
        }
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
    s.as_bytes()
        .windows(needle.len())
        .position(|window| window.eq_ignore_ascii_case(needle.as_bytes()))
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

/// Decode one entity body, the text between `&` and `;`. An unknown one is
/// dropped.
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
    out.push_str(match name {
        "amp" => "&",
        "lt" => "<",
        "gt" => ">",
        "quot" => "\"",
        "apos" => "'",
        "nbsp" => " ",
        _ => "",
    });
}
