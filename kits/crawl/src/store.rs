//! What a crawler read, in its database: `crawl_sites`, `crawl_urls` and
//! `crawl_pages`.

use crate::{page::Fetched, site::Site};
use alloc::{string::String, vec::Vec};
use berm_lang::{CallError, Schema};
use url::Url;

/// A watched site as last read.
#[derive(Schema)]
pub struct Watched {
    pub domain: String,
    /// Its robots.txt, up to 64 KiB; absent when it answered 4xx or was never
    /// read.
    pub robots: Option<String>,
    /// The first `Content-Signal:` its robots.txt carries, as written.
    pub signal: Option<String>,
    /// When it was last read, in Unix milliseconds; 0 if never.
    pub read: u64,
    /// The reads that failed since its last.
    pub failures: u64,
    /// Why its last failed read failed, while `failures` is not 0.
    pub error: Option<String>,
    /// When its last read failed, in Unix milliseconds; 0 if never.
    pub failed: u64,
}

/// A URL a site's sitemaps list, of a kind its program names.
#[derive(Schema)]
pub struct Listed {
    pub url: String,
    pub kind: String,
    /// Its `lastmod`, as the sitemap writes it.
    pub lastmod: Option<String>,
}

/// A page as last fetched.
#[derive(Schema)]
pub struct Page {
    pub url: String,
    /// The status it answered; 0 when nothing was answered.
    pub status: u16,
    /// When it was last fetched, in Unix milliseconds.
    pub fetched: u64,
    pub title: Option<String>,
    pub description: Option<String>,
    /// Up to 64 KiB of its text; absent when it holds none.
    pub text: Option<String>,
    /// Why nothing was answered, when `status` is 0.
    pub error: Option<String>,
}

/// A page due to be fetched, with the validators it last answered with.
pub struct Due {
    pub url: String,
    pub domain: String,
    pub kind: String,
    pub lastmod: Option<String>,
    pub etag: Option<String>,
    pub modified: Option<String>,
    /// The bytes of HTML its program listed it to be read for.
    pub bytes: Option<usize>,
}

/// Watch `domain`, read before the sites not `first` when `first`. Answers
/// whether it was new.
pub(crate) fn watch(domain: &str, first: bool) -> Result<bool, CallError> {
    let known =
        berm_lang::rows::<(u8,)>("SELECT first FROM crawl_sites WHERE domain = ?", &(domain,))?;
    match known.first() {
        Some((was,)) if !first || *was == 1 => Ok(false),
        Some(_) => {
            berm_lang::rows::<()>(
                "UPDATE crawl_sites SET first = 1 WHERE domain = ?",
                &(domain,),
            )?;
            Ok(false)
        }
        None => {
            berm_lang::rows::<()>(
                "INSERT INTO crawl_sites (domain, first) VALUES (?, ?)",
                &(domain, u8::from(first)),
            )?;
            Ok(true)
        }
    }
}

/// Up to `limit` sites never read or last read at `before` or earlier, with
/// fewer than `failures` failed reads in a row, or more with the last failed
/// at `before` or earlier: those watched first before the rest, each least
/// recently read first.
pub(crate) fn sites_due(
    before: u64,
    failures: u64,
    limit: usize,
) -> Result<Vec<String>, CallError> {
    Ok(berm_lang::rows::<(String,)>(
        "SELECT domain FROM crawl_sites WHERE read <= ?1 AND (failures < ?2 OR failed <= ?1)
          ORDER BY first DESC, read LIMIT ?3",
        &(before, failures, limit as u64),
    )?
    .into_iter()
    .map(|(domain,)| domain)
    .collect())
}

/// Store `site` as `domain`'s read at `now`: its sitemaps' URLs in place of
/// the last, and the URLs its program listed that its robots.txt still
/// allows. Answers the bytes written.
pub(crate) fn read(domain: &str, now: u64, site: &Site) -> Result<usize, CallError> {
    berm_lang::rows::<()>(
        "UPDATE crawl_sites SET robots = ?, signal = ?, read = ?, failures = 0, error = NULL
          WHERE domain = ?",
        &(&site.robots, &site.signal, now, domain),
    )?;
    berm_lang::rows::<()>(
        "DELETE FROM crawl_urls WHERE domain = ? AND listed = 0",
        &(domain,),
    )?;
    for (url, kind, lastmod) in &site.urls {
        berm_lang::rows::<()>(
            "INSERT INTO crawl_urls (url, domain, kind, lastmod) VALUES (?, ?, ?, ?)
             ON CONFLICT (url) DO UPDATE SET kind = excluded.kind, lastmod = excluded.lastmod",
            &(url, domain, kind, lastmod),
        )?;
    }
    let listed = berm_lang::rows::<(String,)>(
        "SELECT url FROM crawl_urls WHERE domain = ? AND listed = 1",
        &(domain,),
    )?;
    for (url,) in listed {
        if Url::parse(&url).is_ok_and(|parsed| !site.rules.allows(&parsed)) {
            berm_lang::rows::<()>("DELETE FROM crawl_urls WHERE url = ?", &(&url,))?;
        }
    }
    let urls: usize = site
        .urls
        .iter()
        .map(|(url, kind, lastmod)| {
            url.len() + kind.len() + lastmod.as_ref().map_or(0, String::len)
        })
        .sum();
    Ok(site.robots.as_ref().map_or(0, String::len) + urls)
}

/// Count a failed read of `domain` at `now`, and keep its `error`.
pub(crate) fn read_failed(domain: &str, now: u64, error: &str) -> Result<(), CallError> {
    berm_lang::rows::<()>(
        "UPDATE crawl_sites SET failures = failures + 1, error = ?, failed = ? WHERE domain = ?",
        &(error, now, domain),
    )?;
    Ok(())
}

/// `domain`'s robots.txt as last read; `None` when it answered 4xx or was
/// never read.
pub(crate) fn robots(domain: &str) -> Result<Option<String>, CallError> {
    Ok(berm_lang::rows::<(Option<String>,)>(
        "SELECT robots FROM crawl_sites WHERE domain = ?",
        &(domain,),
    )?
    .into_iter()
    .next()
    .and_then(|(robots,)| robots))
}

/// Keep `url`, of `kind`, as listed for `domain`, a site watched, read for
/// `bytes` of HTML when given. Answers whether it was new.
pub(crate) fn list(
    domain: &str,
    url: &str,
    kind: &str,
    bytes: Option<usize>,
) -> Result<bool, CallError> {
    let known = berm_lang::rows::<(u8,)>("SELECT listed FROM crawl_urls WHERE url = ?", &(url,))?;
    berm_lang::rows::<()>(
        "INSERT INTO crawl_urls (url, domain, kind, listed, bytes) VALUES (?, ?, ?, 1, ?)
         ON CONFLICT (url) DO UPDATE SET listed = 1, bytes = excluded.bytes",
        &(url, domain, kind, bytes.map(|bytes| bytes as u64)),
    )?;
    Ok(known.is_empty())
}

/// Up to `limit` pages due on sites read, those of sites watched first before
/// the rest, a site's least recently fetched before any site's next: never
/// fetched, their URL's `lastmod` changed since, or with no `lastmod` and last
/// fetched at `before` or earlier.
pub(crate) fn pages_due(before: u64, limit: usize) -> Result<Vec<Due>, CallError> {
    Ok(berm_lang::rows::<(
        String,
        String,
        String,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<u64>,
    )>(
        "SELECT u.url, u.domain, u.kind, u.lastmod, p.etag, p.modified, u.bytes
           FROM crawl_urls u JOIN crawl_sites s ON s.domain = u.domain
                LEFT JOIN crawl_pages p ON p.url = u.url
          WHERE s.read > 0
            AND (p.url IS NULL
                 OR (u.lastmod IS NOT NULL AND u.lastmod IS NOT p.lastmod)
                 OR (u.lastmod IS NULL AND p.fetched <= ?))
          ORDER BY s.first DESC,
                   row_number() OVER (PARTITION BY u.domain ORDER BY p.fetched NULLS FIRST),
                   p.fetched NULLS FIRST
          LIMIT ?",
        &(before, limit as u64),
    )?
    .into_iter()
    .map(|(url, domain, kind, lastmod, etag, modified, bytes)| Due {
        url,
        domain,
        kind,
        lastmod,
        etag,
        modified,
        bytes: bytes.map(|bytes| bytes as usize),
    })
    .collect())
}

/// Store what fetching `due` at `now` found. Answers the bytes written.
pub(crate) fn fetched(due: &Due, now: u64, fetched: &Fetched) -> Result<usize, CallError> {
    match fetched {
        Fetched::Unchanged => {
            berm_lang::rows::<()>(
                "UPDATE crawl_pages SET fetched = ?, lastmod = ? WHERE url = ?",
                &(now, &due.lastmod, &due.url),
            )?;
            Ok(0)
        }
        Fetched::Failed(error) => {
            berm_lang::rows::<()>(
                "INSERT INTO crawl_pages (url, status, lastmod, fetched, error) VALUES (?, 0, ?, ?, ?)
                 ON CONFLICT (url) DO UPDATE SET status = 0, lastmod = excluded.lastmod,
                                                 fetched = excluded.fetched, error = excluded.error",
                &(&due.url, &due.lastmod, now, error),
            )?;
            Ok(0)
        }
        Fetched::Answered {
            status,
            etag,
            modified,
            title,
            description,
            text,
            ..
        } => {
            berm_lang::rows::<()>(
                "INSERT OR REPLACE INTO crawl_pages
                    (url, status, etag, modified, lastmod, fetched, title, description, text)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
                &(
                    &due.url,
                    status,
                    etag,
                    modified,
                    &due.lastmod,
                    now,
                    title,
                    description,
                    text,
                ),
            )?;
            Ok([title, description, text]
                .iter()
                .map(|value| value.as_ref().map_or(0, String::len))
                .sum())
        }
    }
}

/// The site `domain` as last read, when it is watched.
pub fn site(domain: &str) -> Result<Option<Watched>, CallError> {
    Ok(berm_lang::rows::<(
        String,
        Option<String>,
        Option<String>,
        u64,
        u64,
        Option<String>,
        u64,
    )>(
        "SELECT domain, robots, signal, read, failures, error, failed FROM crawl_sites
              WHERE domain = ?",
        &(domain,),
    )?
    .pop()
    .map(
        |(domain, robots, signal, read, failures, error, failed)| Watched {
            domain,
            robots,
            signal,
            read,
            failures,
            error,
            failed,
        },
    ))
}

/// Up to `limit` sites whose last read failed at `since` or later, most
/// recently failed first.
pub fn failed(since: u64, limit: usize) -> Result<Vec<Watched>, CallError> {
    Ok(berm_lang::rows::<(
        String,
        Option<String>,
        Option<String>,
        u64,
        u64,
        Option<String>,
        u64,
    )>(
        "SELECT domain, robots, signal, read, failures, error, failed FROM crawl_sites
          WHERE failures > 0 AND failed >= ? ORDER BY failed DESC LIMIT ?",
        &(since, limit as u64),
    )?
    .into_iter()
    .map(
        |(domain, robots, signal, read, failures, error, failed)| Watched {
            domain,
            robots,
            signal,
            read,
            failures,
            error,
            failed,
        },
    )
    .collect())
}

/// The URLs `domain`'s sitemaps list, of `kind` when given, in URL order.
pub fn urls(domain: &str, kind: Option<&str>) -> Result<Vec<Listed>, CallError> {
    Ok(berm_lang::rows::<(String, String, Option<String>)>(
        "SELECT url, kind, lastmod FROM crawl_urls
          WHERE domain = ? AND (? IS NULL OR kind = ?) ORDER BY url",
        &(domain, kind, kind),
    )?
    .into_iter()
    .map(|(url, kind, lastmod)| Listed { url, kind, lastmod })
    .collect())
}

/// The page at `url` as last fetched.
pub fn page(url: &str) -> Result<Option<Page>, CallError> {
    Ok(berm_lang::rows::<(
        String,
        u16,
        u64,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
    )>(
        "SELECT url, status, fetched, title, description, text, error FROM crawl_pages
          WHERE url = ?",
        &(url,),
    )?
    .pop()
    .map(
        |(url, status, fetched, title, description, text, error)| Page {
            url,
            status,
            fetched,
            title,
            description,
            text,
            error,
        },
    ))
}
