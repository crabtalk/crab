//! The reference crawler: the sites it watches, read through `crab-crawl`.
//!
//! `watch` watches domains, `tick` reads the sites and fetches the pages due,
//! and `urls` and `page` read back what was kept.

#![cfg_attr(target_arch = "wasm32", no_std, no_main)]

extern crate alloc;

use crawl::{Crawler, Schedule};
use url::Url;

#[berm_lang::program(uses = [crawl])]
mod tools {
    use crate::Pages;
    use alloc::{string::String, vec::Vec};
    use berm_lang::Result;
    use crawl::store::{self, Listed, Page};

    /// Watches `domains`, and answers how many were new.
    #[public]
    pub fn watch(domains: Vec<String>) -> Result<u64> {
        Ok(crawl::watch(&domains)? as u64)
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
}

/// Pricing and comparison pages, a site read again after a week and a page
/// with no `lastmod` after a month.
pub struct Pages;

impl Crawler for Pages {
    const SCHEDULE: Schedule = Schedule {
        recrawl: 7 * DAY,
        refetch: 30 * DAY,
        sites: 10,
        pages: 10,
        failures: 2,
    };
    const AGENT: &'static str = "berm-site/0.1";

    fn kind(_domain: &str, url: &Url) -> Option<&'static str> {
        match url.path() {
            "/pricing" => Some("pricing"),
            path if path.starts_with("/compare/") => Some("comparison"),
            _ => None,
        }
    }
}

/// One day, in milliseconds.
const DAY: u64 = 24 * 60 * 60 * 1000;
