//! An incremental crawler as a kit: the domains its program watches, each
//! site's robots.txt and sitemaps, and the pages of the kinds the program
//! names, fetched on its schedule into its database. Its tables are
//! `crawl_sites`, `crawl_urls` and `crawl_pages`.
//!
//! A tick reads the sites due, least recently read first: robots.txt, then the
//! sitemaps it names, else `/sitemap.xml`, keeping each listed URL of a kind
//! that robots.txt allows in place of the site's last. It then fetches the
//! pages due, least recently fetched first: new, their `lastmod` changed, or
//! with none and fetched longer ago than the refetch interval, each
//! conditionally on the validators it last answered with. A tick stops at its
//! byte budget and leaves the rest due; all of it commits with the call.

#![no_std]

extern crate alloc;

use alloc::{string::String, vec::Vec};
use berm_lang::{CallError, Result, event};
use url::Url;

mod fetch;
mod page;
mod robots;
mod site;
mod sitemap;
pub mod store;

berm_lang::kit!(
    syscalls = { "berm.http.fetch" = ["*"] },
    database = "migrations",
);

/// Bytes a tick stores before it reads and fetches no more.
const BUDGET: usize = 8 << 20;

/// When a crawler reads and fetches. Times are in milliseconds.
pub struct Schedule {
    /// Before a site's robots.txt and sitemaps are read again.
    pub recrawl: u64,
    /// Before a page whose URL has no `lastmod` is fetched again.
    pub refetch: u64,
    /// Sites read per tick.
    pub sites: usize,
    /// Pages fetched per tick.
    pub pages: usize,
    /// Failed reads in a row after which a site is read no more.
    pub failures: u64,
}

/// A crawler: what it sends and which pages it keeps.
pub trait Crawler {
    const SCHEDULE: Schedule;
    /// Its `User-Agent`. The part before the first `/` is the token its
    /// robots.txt group is matched by, before `*`.
    const AGENT: &'static str;
    /// The kind of `url`, listed by `domain`'s sitemaps, when it is a page
    /// fetched.
    fn kind(domain: &str, url: &Url) -> Option<&'static str>;
}

/// A site read in a tick.
#[event]
pub struct Read {
    pub domain: String,
    /// How many URLs of a kind its sitemaps listed.
    pub urls: u64,
}

/// A site whose read failed in a tick; it is read again on a later tick.
#[event]
pub struct ReadFailed {
    pub domain: String,
    pub error: String,
}

/// A page whose fetch failed in a tick; it is fetched again when next due.
#[event]
pub struct FetchFailed {
    pub url: String,
    pub error: String,
}

/// Watch `domains`, each a host name; one already watched is left as it is.
/// Answers how many were new.
pub fn watch(domains: &[String]) -> Result<usize, CallError> {
    domains.iter().try_fold(0, |new, domain| {
        Ok(new + usize::from(store::watch(&domain.to_ascii_lowercase())?))
    })
}

/// One tick of `C`.
pub fn tick<C: Crawler>() -> Result<()> {
    let schedule = C::SCHEDULE;
    let now = berm_lang::now();
    let mut stored = 0;
    let due = store::sites_due(
        now.saturating_sub(schedule.recrawl),
        schedule.failures,
        schedule.sites,
    )?;
    for domain in due {
        if stored >= BUDGET {
            break;
        }
        match site::read::<C>(&domain) {
            Ok(site) => {
                stored += store::read(&domain, now, &site)?;
                berm_lang::emit!(Read {
                    domain,
                    urls: site.urls.len() as u64,
                })?;
            }
            Err(error) => {
                store::failed(&domain)?;
                berm_lang::emit!(ReadFailed { domain, error })?;
            }
        }
    }
    let due: Vec<store::Due> =
        store::pages_due(now.saturating_sub(schedule.refetch), schedule.pages)?;
    for page in due {
        if stored >= BUDGET {
            break;
        }
        let fetched = page::fetch::<C>(&page);
        if let page::Fetched::Failed(error) = &fetched {
            berm_lang::emit!(FetchFailed {
                url: page.url.clone(),
                error: error.clone(),
            })?;
        }
        stored += store::fetched(&page, now, &fetched)?;
    }
    Ok(())
}
