//! The reference crawler: the sites it watches, read through `crabkit-crawl`.
//!
//! `watch` watches domains, `list` lists URLs on a site, `tick` reads the sites
//! and fetches the pages due, and `urls` and `page` read back what was kept,
//! and `read` what the crawler read of a page. The crawler lists the links of
//! each page it reads.

#![cfg_attr(target_arch = "wasm32", no_std)]

extern crate alloc;

use alloc::{format, string::String, vec::Vec};
use berm_lang::{CallError, Result, Schema, tool};
use crawl::{
    Crawler, Schedule,
    store::{self, Listed, Page},
};
use url::Url;

/// Watches `domains`, and answers how many were new.
#[tool(public)]
pub fn watch(domains: Vec<String>) -> Result<u64> {
    Ok(crawl::watch(&domains)? as u64)
}

/// Watches `domains` to be read before the other sites due, and answers
/// how many were new.
#[tool(public)]
pub fn watch_first(domains: Vec<String>) -> Result<u64> {
    Ok(crawl::watch_first(&domains)? as u64)
}

/// Lists `urls` on `domain`'s site, each read for `bytes` of HTML when
/// given, and answers how many were new.
#[tool(public)]
pub fn list(domain: String, urls: Vec<String>, bytes: Option<u64>) -> Result<u64> {
    Ok(crawl::list::<Pages>(&domain, &urls, bytes.map(|bytes| bytes as usize))? as u64)
}

/// Reads the sites due and fetches the pages due.
#[tool(cron)]
pub fn tick() -> Result<()> {
    crawl::tick::<Pages>()
}

/// The URLs `domain`'s sitemaps list of a kind kept.
#[tool(query, public)]
pub fn urls(domain: String) -> Result<Vec<Listed>> {
    Ok(store::urls(&domain, None)?)
}

/// The page at `url` as last fetched.
#[tool(query, public)]
pub fn page(url: String) -> Result<Option<Page>> {
    Ok(store::page(&url)?)
}

/// What the crawler last read at `url`.
#[tool(query, public)]
pub fn read(url: String) -> Result<Option<Read>> {
    Ok(berm_lang::get_json(&crate::read_key(&url))?)
}

/// Pricing and comparison pages, a site read again after a week and a page
/// with no `lastmod` after a month.
pub struct Pages;

impl Crawler for Pages {
    const SCHEDULE: Schedule = Schedule {
        recrawl: 7 * DAY,
        refetch: 30 * DAY,
        sites: 10,
        sitemaps: 4 << 20,
        pages: 10,
        html: 2 << 20,
        page: 256 << 10,
        failures: 2,
        timeout: 10_000,
    };
    const AGENT: &'static str = "berm-site/0.1";

    fn kind(_domain: &str, url: &Url) -> Option<&'static str> {
        match url.path() {
            "/pricing" => Some("pricing"),
            path if path.starts_with("/compare/") => Some("comparison"),
            _ => None,
        }
    }

    fn read(domain: &str, url: &str, kind: &str, html: &str) -> Result<usize, CallError> {
        let read = Read {
            kind: String::from(kind),
            length: html.len() as u64,
        };
        berm_lang::set_json(&read_key(url), &read)?;
        let base = Url::parse(url).map_err(|error| CallError::Failed(format!("{url}: {error}")))?;
        let links: Vec<String> = html::links(html)
            .into_iter()
            .filter_map(|link| base.join(&link.href).ok())
            .map(String::from)
            .collect();
        crawl::list::<Pages>(domain, &links, None)?;
        Ok(0)
    }
}

/// The `berm.set` key of what the crawler read at `url`.
fn read_key(url: &str) -> String {
    format!("read:{url}")
}

/// A page as the crawler read it: its kind and its HTML's length.
#[derive(Schema)]
pub struct Read {
    pub kind: String,
    pub length: u64,
}

/// One day, in milliseconds.
const DAY: u64 = 24 * 60 * 60 * 1000;
