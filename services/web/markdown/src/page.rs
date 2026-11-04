use berm_lang::{
    Result,
    http::{self, Response},
};
use scraper::{Html, Node, Selector};
use std::collections::BTreeSet;
use url::Url;

pub const MAX_HTML: usize = 512 * 1024;
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
        let mut document = Html::parse_document(source);
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
