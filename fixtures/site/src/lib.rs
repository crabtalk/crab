//! The reference crawler: the sites it watches, read through `crab-crawl`.
//!
//! `watch` watches domains, `tick` reads the sites and fetches the pages due,
//! and `urls` and `page` read back what was kept, and `read` what the crawler
//! read of a page.

#![cfg_attr(target_arch = "wasm32", no_std, no_main)]

extern crate alloc;

use alloc::{format, string::String};
use berm_lang::{CallError, Schema};
use crawl::{Crawler, Schedule};
use url::Url;

#[berm_lang::program(uses = [crawl])]
mod tools {
    use crate::{Pages, Read};
    use alloc::{string::String, vec::Vec};
    use berm_lang::Result;
    use crawl::store::{self, Listed, Page};

    /// Watches `domains`, and answers how many were new.
    #[public]
    pub fn watch(domains: Vec<String>) -> Result<u64> {
        Ok(crawl::watch(&domains)? as u64)
    }

    /// Watches `domains` to be read before the other sites due, and answers
    /// how many were new.
    #[public]
    pub fn watch_first(domains: Vec<String>) -> Result<u64> {
        Ok(crawl::watch_first(&domains)? as u64)
    }

    /// Reads the sites due and fetches the pages due.
    #[cron]
    pub fn tick() -> Result<()> {
        crawl::tick::<Pages>()
    }

    /// The URLs `domain`'s sitemaps list of a kind kept.
    #[query]
    #[public]
    pub fn urls(domain: String) -> Result<Vec<Listed>> {
        Ok(store::urls(&domain, None)?)
    }

    /// The page at `url` as last fetched.
    #[query]
    #[public]
    pub fn page(url: String) -> Result<Option<Page>> {
        Ok(store::page(&url)?)
    }

    /// What the crawler last read at `url`.
    #[query]
    #[public]
    pub fn read(url: String) -> Result<Option<Read>> {
        Ok(berm_lang::get_json(&crate::read_key(&url))?)
    }
}

/// Pricing and comparison pages, a site read again after a week and a page
/// with no `lastmod` after a month.
pub struct Pages;

impl Crawler for Pages {
    const SCHEDULE: Schedule = Schedule {
        recrawl: 7 * DAY,
        refetch: 30 * DAY,
        sites: 10,
        listed: 20_000,
        pages: 10,
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

    fn read(_domain: &str, url: &str, kind: &str, html: &str) -> Result<usize, CallError> {
        let read = Read {
            kind: String::from(kind),
            length: html.len() as u64,
        };
        berm_lang::set_json(&read_key(url), &read)?;
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
