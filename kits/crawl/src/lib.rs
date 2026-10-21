//! An incremental crawler as a kit: the domains its program watches, each
//! site's robots.txt and sitemaps, and the pages of the kinds the program
//! names, fetched on its schedule into its database. Its tables are
//! `crawl_sites`, `crawl_urls` and `crawl_pages`.
//!
//! A tick reads the sites due, those watched first before the rest, least
//! recently read first: robots.txt, then the sitemaps it names, else
//! `/sitemap.xml`, keeping each listed URL of a kind that robots.txt allows in
//! place of the site's last. It then fetches the pages due, those of sites
//! watched first before the rest, a site's least recently fetched before any
//! site's next: new, their `lastmod` changed, or with none and fetched longer
//! ago than the refetch interval, each conditionally on the validators it last
//! answered with, and hands each page that answered with HTML to the crawler to
//! read. Its sites are read at once, and its pages fetched a page a site a
//! round, each round's requests sent as one batch, so a site sees one request
//! at a time. A tick stops at its byte budget and leaves the rest due; all of
//! it commits with the call.

#![no_std]

extern crate alloc;

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
    /// Bytes of sitemaps a site is read for. The first site due is read for
    /// this many of its own; the rest of a tick's share this many, and one
    /// whose sitemaps would take more than are left is left due.
    pub sitemaps: usize,
    /// Pages fetched per tick.
    pub pages: usize,
    /// Bytes of HTML a tick reads before it fetches no more pages. A page's
    /// HTML costs about 114 wasm instructions a byte to read (measured
    /// 2026-10-21), about 11ns of CPU at 10,000 instructions a microsecond.
    pub html: usize,
    /// Bytes of HTML a page is read for. A round fetches no more pages than
    /// the HTML left holds at this many each.
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
    domains.iter().try_fold(0, |new, domain| {
        Ok(new + usize::from(store::watch(&domain.to_ascii_lowercase(), false)?))
    })
}

/// [`watch`] `domains`, each read before the other sites due, one already
/// watched included. Answers how many were new.
pub fn watch_first(domains: &[String]) -> Result<usize, CallError> {
    domains.iter().try_fold(0, |new, domain| {
        Ok(new + usize::from(store::watch(&domain.to_ascii_lowercase(), true)?))
    })
}

/// Keep each of `urls` on `domain`'s site, a site watched, that is of a kind
/// `C` names, to be fetched as its sitemaps' URLs are once the site is read,
/// under its robots.txt. A sitemap read leaves them; a robots.txt that
/// disallows one drops it. `Crawler::read` may list the links of the page it
/// is handed, in the same call. Answers how many were new.
pub fn list<C: Crawler>(domain: &str, urls: &[String]) -> Result<usize, CallError> {
    let domain = domain.to_ascii_lowercase();
    urls.iter().try_fold(0, |new, listed| {
        let Ok(url) = Url::parse(listed) else {
            return Ok(new);
        };
        let on_site = url.host_str().is_some_and(|host| same_site(host, &domain));
        match C::kind(&domain, &url) {
            Some(kind) if on_site => {
                Ok(new + usize::from(store::list(&domain, url.as_str(), kind)?))
            }
            _ => Ok(new),
        }
    })
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
        // A page a site a round.
        let left = schedule.html - read;
        let width = (left / schedule.page).max(1);
        let mut round: Vec<store::Due> = Vec::new();
        let mut later: VecDeque<store::Due> = VecDeque::new();
        for page in due.drain(..) {
            if silent.contains(&page.domain) {
                continue;
            }
            match round.len() == width || round.iter().any(|taken| taken.domain == page.domain) {
                true => later.push_back(page),
                false => round.push(page),
            }
        }
        due = later;
        if round.is_empty() {
            break;
        }
        let pages: Vec<&store::Due> = round.iter().collect();
        let fetched = page::fetch_all::<C>(&pages, schedule.page.min(left));
        for (page, fetched) in round.iter().zip(fetched) {
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
