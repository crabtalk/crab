use berm_lang::{
    Result,
    http::{self, Response},
};
use html5ever::{driver, tendril::TendrilSink};
use lol_html::{HtmlRewriter, Settings, element};
use scraper::{Html, HtmlTreeSink, Node, Selector};
use std::collections::BTreeSet;
use url::Url;

pub const MAX_HTML: usize = 512 * 1024;
pub const MAX_NODES: usize = 12_000;
pub const MAX_DEPTH: usize = 128;
pub const MAX_ATTRIBUTES: usize = 128;
pub const MAX_TOTAL_ATTRIBUTES: usize = 12_000;
pub const MAX_TAGS: usize = 12_000;
const PARSE_CHUNK: usize = 1024;
const MAX_URL: usize = 4096;
const TIMEOUT: u64 = 10_000;
const REDIRECTS: usize = 3;
const ACCEPT: &str = "text/html, application/xhtml+xml";

pub struct Page {
    pub url: Url,
    pub base: Url,
    pub response: Response,
    pub document: Html,
}

impl Page {
    fn check_complexity(document: &Html, seen: usize) -> Result<usize> {
        let count = document.tree.nodes().len();
        if count > MAX_NODES {
            return Err(format!("The page exceeds the HTML node limit ({MAX_NODES})").into());
        }
        if document
            .tree
            .nodes()
            .rev()
            .take(count - seen)
            .any(|node| node.ancestors().take(MAX_DEPTH + 1).count() > MAX_DEPTH)
        {
            return Err(format!("The page exceeds the HTML nesting limit ({MAX_DEPTH})").into());
        }
        Ok(count)
    }

    pub fn fetch(given: &str) -> Result<Self> {
        let mut url = Url::parse(given).map_err(|_| "Provide an absolute HTTP or HTTPS URL")?;
        if given.len() > MAX_URL {
            return Err("The URL exceeds 4096 bytes".into());
        }
        let mut seen = BTreeSet::new();
        for hop in 0..=REDIRECTS {
            if !matches!(url.scheme(), "http" | "https")
                || !url.username().is_empty()
                || url.password().is_some()
            {
                return Err("Use HTTP or HTTPS without embedded credentials".into());
            }
            url.set_fragment(None);
            if !seen.insert(url.clone()) {
                return Err("The page redirects in a loop".into());
            }
            let response = http::fetch_all(&[http::Request {
                method: "GET",
                url: url.as_str(),
                headers: &[("Accept", ACCEPT)],
                body: &[],
                timeout: Some(TIMEOUT),
                limit: Some(MAX_HTML + 1),
            }])?
            .pop()
            .ok_or("The host returned no page response")??;
            if matches!(response.status, 301 | 302 | 303 | 307 | 308) {
                if hop == REDIRECTS {
                    return Err("The page exceeds three redirects".into());
                }
                let location = response
                    .header("location")
                    .ok_or("The redirect has no Location header")?;
                url = url
                    .join(location)
                    .map_err(|_| "The redirect has an invalid Location")?;
                continue;
            }
            return Self::try_from((url, response));
        }
        unreachable!()
    }

    pub fn title(&self) -> Option<String> {
        self.document
            .select(&Selector::parse("title").expect("static selector"))
            .next()
            .map(|element| element.text().collect::<String>().trim().to_owned())
            .filter(|title| !title.is_empty())
    }

    pub fn meta(&self, name: &str) -> Option<&str> {
        self.document
            .select(&Selector::parse("meta").expect("static selector"))
            .filter(|element| {
                ["name", "property"].iter().any(|key| {
                    element
                        .value()
                        .attr(key)
                        .is_some_and(|value| value.eq_ignore_ascii_case(name))
                })
            })
            .find_map(|element| {
                element
                    .value()
                    .attr("content")
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
            })
    }
}

impl TryFrom<(Url, Response)> for Page {
    type Error = berm_lang::Error;

    fn try_from((url, response): (Url, Response)) -> Result<Self> {
        if !response.is_success() {
            return Err(format!("The page returned HTTP {}", response.status).into());
        }
        let media = response
            .header("content-type")
            .unwrap_or("")
            .split(';')
            .next()
            .unwrap_or("")
            .trim();
        if !media.eq_ignore_ascii_case("text/html")
            && !media.eq_ignore_ascii_case("application/xhtml+xml")
        {
            return Err("The URL must return HTML (text/html or application/xhtml+xml)".into());
        }
        if response.body.len() > MAX_HTML {
            return Err("The page exceeds the 512 KiB HTML limit".into());
        }
        let source =
            std::str::from_utf8(&response.body).map_err(|_| "The page is not UTF-8 HTML")?;
        let mut tags = 0;
        let mut attributes = 0;
        let mut validator = HtmlRewriter::new(
            Settings::new().append_element_content_handler(element!("*", |element| {
                tags += 1;
                if tags > MAX_TAGS {
                    return Err(format!("The page exceeds the HTML tag limit ({MAX_TAGS})").into());
                }
                attributes += element.attributes().len();
                if attributes > MAX_TOTAL_ATTRIBUTES {
                    return Err(format!(
                        "The page exceeds the total HTML attribute limit ({MAX_TOTAL_ATTRIBUTES})"
                    )
                    .into());
                }
                if element.attributes().len() > MAX_ATTRIBUTES {
                    return Err(format!(
                        "The page exceeds the HTML attribute limit ({MAX_ATTRIBUTES} per element)"
                    )
                    .into());
                }
                Ok(())
            })),
            |_: &[u8]| {},
        );
        validator
            .write(source.as_bytes())
            .map_err(|error| format!("Cannot parse HTML: {error}"))?;
        validator
            .end()
            .map_err(|error| format!("Cannot parse HTML: {error}"))?;
        let mut parser =
            driver::parse_document(HtmlTreeSink::new(Html::new_document()), Default::default());
        let mut remaining = source;
        let mut seen = 0;
        while !remaining.is_empty() {
            let end = remaining.floor_char_boundary(PARSE_CHUNK);
            parser.process(remaining[..end].into());
            remaining = &remaining[end..];
            seen = Self::check_complexity(&parser.tokenizer.sink.sink.0.borrow(), seen)?;
        }
        let mut document = parser.finish();
        Self::check_complexity(&document, 0)?;
        let base = document
            .select(&Selector::parse("base[href]").expect("static selector"))
            .next()
            .and_then(|element| url.join(element.value().attr("href")?).ok())
            .filter(|base| matches!(base.scheme(), "http" | "https"))
            .unwrap_or_else(|| url.clone());
        let linked: Vec<_> = document
            .select(&Selector::parse("[href], [src]").expect("static selector"))
            .map(|element| element.id())
            .collect();
        for id in linked {
            let mut node = document.tree.get_mut(id).expect("selected element");
            if let Node::Element(element) = node.value() {
                element.attrs.retain_mut(|(name, value)| {
                    if !matches!(name.local.as_ref(), "href" | "src") {
                        return true;
                    }
                    match base.join(value) {
                        Ok(target)
                            if matches!(target.scheme(), "http" | "https" | "mailto" | "tel") =>
                        {
                            *value = target.as_str().into();
                            true
                        }
                        _ => false,
                    }
                });
            }
        }
        Ok(Self {
            url,
            base,
            response,
            document,
        })
    }
}
