//! The fetches of pages due, a batch at a time.

use crate::{
    Crawler,
    fetch::{self, Get},
    store::Due,
};
use alloc::{format, string::String, vec, vec::Vec};
use berm_lang::http::Response;
use url::Url;

/// The most of a page's text kept, in bytes.
const TEXT: usize = 64 << 10;

/// What a fetch of a page found.
pub enum Fetched {
    /// It answered `304`: what is stored stands.
    Unchanged,
    Answered {
        status: u16,
        etag: Option<String>,
        modified: Option<String>,
        title: Option<String>,
        description: Option<String>,
        /// Its text, cut at [`TEXT`] bytes; `None` when it holds no text or
        /// names a `Content-Type` other than HTML.
        text: Option<String>,
        /// The HTML read of it, when it answered `200` with HTML.
        html: Option<String>,
    },
    /// Nothing was answered.
    Failed(String),
}

/// Fetch each of `dues` as `C`, at once, each conditionally on the validators
/// it last answered with and read for the first bytes paired with it, and
/// answer each in its place.
pub fn fetch_all<C: Crawler>(dues: &[(Due, usize)]) -> Vec<Fetched> {
    let mut gets: Vec<Get<'_>> = Vec::new();
    let mut parsed: Vec<Result<(), String>> = Vec::new();
    for (due, limit) in dues {
        match Url::parse(&due.url) {
            Ok(url) => {
                let mut headers = vec![("Accept", "text/html")];
                if let Some(etag) = &due.etag {
                    headers.push(("If-None-Match", etag.as_str()));
                }
                if let Some(modified) = &due.modified {
                    headers.push(("If-Modified-Since", modified.as_str()));
                }
                gets.push(Get {
                    url,
                    domain: &due.domain,
                    headers,
                    limit: *limit,
                });
                parsed.push(Ok(()));
            }
            Err(error) => parsed.push(Err(format!("{}: {error}", due.url))),
        }
    }
    let mut answers = fetch::get_all(&gets, C::AGENT, C::SCHEDULE.timeout).into_iter();
    parsed
        .into_iter()
        .map(|parsed| match parsed {
            Ok(()) => match answers.next() {
                Some(Ok(answer)) => read(answer),
                Some(Err(error)) => Fetched::Failed(error),
                None => Fetched::Failed(String::from("no answer")),
            },
            Err(error) => Fetched::Failed(error),
        })
        .collect()
}

/// What `answer` holds.
fn read(answer: Response) -> Fetched {
    let status = answer.status;
    if status == 304 {
        return Fetched::Unchanged;
    }
    let etag = answer.header("etag").map(String::from);
    let modified = answer.header("last-modified").map(String::from);
    let is_html = answer
        .header("content-type")
        .is_none_or(|kind| kind.to_ascii_lowercase().contains("html"));
    if status != 200 || !is_html {
        return Fetched::Answered {
            status,
            etag,
            modified,
            title: None,
            description: None,
            text: None,
            html: None,
        };
    }
    let page = String::from_utf8(answer.body)
        .unwrap_or_else(|error| String::from_utf8_lossy(error.as_bytes()).into_owned());
    let mut text = html::text(&page);
    text.truncate(text.floor_char_boundary(TEXT));
    let html::Head { title, metas } =
        html::head(&page, &["og:title", "description", "og:description"]);
    let [og_title, description, og_description] = <[_; 3]>::try_from(metas).unwrap_or_default();
    Fetched::Answered {
        status,
        etag,
        modified,
        title: title.or(og_title),
        description: description.or(og_description),
        text: (!text.is_empty()).then_some(text),
        html: Some(page),
    }
}
