//! One fetch of a page due.

use crate::{Crawler, fetch, store::Due};
use alloc::{format, string::String};
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
        /// Up to 64 KiB of its text; `None` when it holds no text or names
        /// a `Content-Type` other than HTML.
        text: Option<String>,
    },
    /// Nothing was answered.
    Failed(String),
}

/// Fetch `due` as `C`, conditionally on the validators it last answered with.
pub fn fetch<C: Crawler>(due: &Due) -> Fetched {
    let url = match Url::parse(&due.url) {
        Ok(url) => url,
        Err(error) => return Fetched::Failed(format!("{}: {error}", due.url)),
    };
    let mut headers = alloc::vec![("Accept", "text/html")];
    if let Some(etag) = &due.etag {
        headers.push(("If-None-Match", etag));
    }
    if let Some(modified) = &due.modified {
        headers.push(("If-Modified-Since", modified));
    }
    let answer = match fetch::get(&url, &due.domain, C::AGENT, &headers) {
        Ok(answer) => answer,
        Err(error) => return Fetched::Failed(error),
    };
    let status = answer.status;
    if status == 304 {
        return Fetched::Unchanged;
    }
    let etag = fetch::header(&answer, "etag").map(String::from);
    let modified = fetch::header(&answer, "last-modified").map(String::from);
    let is_html = fetch::header(&answer, "content-type")
        .is_none_or(|kind| kind.to_ascii_lowercase().contains("html"));
    if status != 200 || !is_html {
        return Fetched::Answered {
            status,
            etag,
            modified,
            title: None,
            description: None,
            text: None,
        };
    }
    let page = String::from_utf8_lossy(&answer.body);
    let mut text = html::text(&page);
    text.truncate(text.floor_char_boundary(TEXT));
    Fetched::Answered {
        status,
        etag,
        modified,
        title: html::title(&page).or_else(|| html::meta(&page, "og:title")),
        description: html::meta(&page, "description")
            .or_else(|| html::meta(&page, "og:description")),
        text: (!text.is_empty()).then_some(text),
    }
}
