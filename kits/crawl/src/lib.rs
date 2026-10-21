//! An incremental crawler as a kit: the domains its program watches, each
//! site's robots.txt and sitemaps, and the pages of the kinds the program
//! names, fetched on its schedule into its database. Its tables are
//! `crawl_sites`, `crawl_urls` and `crawl_pages`.
//!
//! A site sees one request at a time. A tick stops at its byte budget and
//! leaves the rest due; all of it commits with the call.

#![no_std]

extern crate alloc;

use crate::robots::Robots;
use alloc::{collections::VecDeque, string::String, vec::Vec};
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
    /// Bytes of sitemaps the first site due in a tick is read for; the rest
    /// of the tick's sites share as many. A site cut at its share is left due.
    pub sitemaps: usize,
    /// Pages fetched per tick.
    pub pages: usize,
    /// Bytes of HTML a tick reads before it fetches no more pages. A page's
    /// HTML costs about 114 wasm instructions a byte to read (measured
    /// 2026-10-21), about 11ns of CPU at 10,000 instructions a microsecond.
    pub html: usize,
    /// Bytes of HTML a page is read for, unless its program listed it with
    /// its own. A page listed with more than the tick's HTML has left is left
    /// due, unless it is the tick's first, which is read past `html`.
    pub page: usize,
    /// Failed reads in a row after which a site is read again only `recrawl`
    /// after its last failed.
    pub failures: u64,
    /// Milliseconds a request waits for an answer before it fails.
    pub timeout: u64,
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

    /// Read `html`, the page at `url` of `kind` on `domain`, fetched answering
    /// `200` with HTML, in the call that stored it. Answers the bytes it
    /// stored, which count toward the tick's budget. Stores nothing unless
    /// implemented.
    fn read(_domain: &str, _url: &str, _kind: &str, _html: &str) -> Result<usize, CallError> {
        Ok(0)
    }
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
    store::watch(domains, false)
}

/// [`watch`] `domains`, each read before the other sites due, one already
/// watched included. Answers how many were new.
pub fn watch_first(domains: &[String]) -> Result<usize, CallError> {
    store::watch(domains, true)
}

/// Keep each of `urls` on `domain`'s site, a site watched, that is of a kind
/// `C` names, to be fetched as its sitemaps' URLs are once the site is read,
/// under its robots.txt, each read for `bytes` of HTML when given, else
/// `Schedule::page`. A sitemap read leaves them; a robots.txt that disallows
/// one drops it. `Crawler::read` may list the links of the page it is handed,
/// in the same call. Answers how many were new.
pub fn list<C: Crawler>(
    domain: &str,
    urls: &[String],
    bytes: Option<usize>,
) -> Result<usize, CallError> {
    let domain = domain.to_ascii_lowercase();
    let robots = Robots::parse(&store::robots(&domain)?.unwrap_or_default(), C::AGENT);
    let listed: Vec<(String, &str)> = urls
        .iter()
        .filter_map(|listed| {
            let url = Url::parse(listed).ok()?;
            let on_site = url.host_str().is_some_and(|host| same_site(host, &domain));
            if !on_site || !robots.allows(&url) {
                return None;
            }
            let kind = C::kind(&domain, &url)?;
            Some((String::from(url), kind))
        })
        .collect();
    store::list(&domain, &listed, bytes)
}

/// Whether `host` belongs to `domain`'s site: either is the other or a
/// subdomain of it.
pub fn same_site(host: &str, domain: &str) -> bool {
    let within = |inner: &str, outer: &str| {
        outer.contains('.')
            && (inner == outer
                || inner
                    .strip_suffix(outer)
                    .is_some_and(|prefix| prefix.ends_with('.')))
    };
    within(host, domain) || within(domain, host)
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
    let reads = site::read_all::<C>(&due, schedule.sitemaps);
    for (domain, read) in due.into_iter().zip(reads) {
        match read {
            Ok(None) => {}
            Ok(Some(site)) => {
                stored += store::read(&domain, now, &site)?;
                berm_lang::emit!(Read {
                    domain,
                    urls: site.urls.len() as u64,
                })?;
            }
            Err(error) => {
                store::read_failed(&domain, now, &error)?;
                berm_lang::emit!(ReadFailed { domain, error })?;
            }
        }
    }
    let mut due: VecDeque<store::Due> =
        store::pages_due(now.saturating_sub(schedule.refetch), schedule.pages)?.into();
    let mut read = 0;
    // The sites that answered a page with nothing this tick.
    let mut silent: Vec<String> = Vec::new();
    while !due.is_empty() && stored < BUDGET && read < schedule.html {
        // A page a site a round, each with the bytes it is read for.
        let mut left = schedule.html - read;
        let mut round: Vec<(store::Due, usize)> = Vec::new();
        let mut later: VecDeque<store::Due> = VecDeque::new();
        for page in due.drain(..) {
            if silent.contains(&page.domain) {
                continue;
            }
            let bytes = page.bytes.unwrap_or(schedule.page);
            let bytes = if round.iter().any(|(taken, _)| taken.domain == page.domain) {
                None
            } else if bytes <= left {
                Some(bytes)
            } else if !round.is_empty() {
                None
            } else if page.bytes.is_none() {
                Some(left)
            } else {
                (read == 0).then_some(bytes)
            };
            match bytes {
                Some(bytes) => {
                    left = left.saturating_sub(bytes);
                    round.push((page, bytes));
                }
                None => later.push_back(page),
            }
        }
        due = later;
        if round.is_empty() {
            break;
        }
        let fetched = page::fetch_all::<C>(&round);
        for ((page, _), fetched) in round.iter().zip(fetched) {
            if let page::Fetched::Failed(error) = &fetched {
                silent.push(page.domain.clone());
                berm_lang::emit!(FetchFailed {
                    url: page.url.clone(),
                    error: error.clone(),
                })?;
            }
            stored += store::fetched(page, now, &fetched)?;
            if let page::Fetched::Answered {
                html: Some(html), ..
            } = &fetched
            {
                read += html.len();
                stored += C::read(&page.domain, &page.url, &page.kind, html)?;
            }
        }
    }
    Ok(())
}
