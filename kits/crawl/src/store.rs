//! What a crawler read, in its database: `crawl_sites`, `crawl_urls` and
//! `crawl_pages`.

use crate::{page::Fetched, site::Site};
use alloc::{string::String, vec::Vec};
use berm_lang::{CallError, Schema};

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
}

/// A page due to be fetched, with the validators it last answered with.
pub struct Due {
    pub url: String,
    pub domain: String,
    pub lastmod: Option<String>,
    pub etag: Option<String>,
    pub modified: Option<String>,
}

/// Watch `domain`. Answers whether it was new.
pub(crate) fn watch(domain: &str) -> Result<bool, CallError> {
    let added = berm_lang::rows::<(u8,)>(
        "INSERT OR IGNORE INTO crawl_sites (domain) VALUES (?) RETURNING 1",
        &(domain,),
    )?;
    Ok(!added.is_empty())
}

/// Up to `limit` sites never read or last read at `before` or earlier, with
/// fewer than `failures` failed reads, least recently read first.
pub(crate) fn sites_due(
    before: u64,
    failures: u64,
    limit: usize,
) -> Result<Vec<String>, CallError> {
    Ok(berm_lang::rows::<(String,)>(
        "SELECT domain FROM crawl_sites WHERE read <= ? AND failures < ? ORDER BY read LIMIT ?",
        &(before, failures, limit as u64),
    )?
    .into_iter()
    .map(|(domain,)| domain)
    .collect())
}

/// Store `site` as `domain`'s read at `now`, its URLs in place of the last.
/// Answers the bytes written.
pub(crate) fn read(domain: &str, now: u64, site: &Site) -> Result<usize, CallError> {
    berm_lang::rows::<()>(
        "UPDATE crawl_sites SET robots = ?, signal = ?, read = ?, failures = 0 WHERE domain = ?",
        &(&site.robots, &site.signal, now, domain),
    )?;
    berm_lang::rows::<()>("DELETE FROM crawl_urls WHERE domain = ?", &(domain,))?;
    for (url, kind, lastmod) in &site.urls {
        berm_lang::rows::<()>(
            "INSERT OR REPLACE INTO crawl_urls (url, domain, kind, lastmod) VALUES (?, ?, ?, ?)",
            &(url, domain, kind, lastmod),
        )?;
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

/// Count a failed read of `domain`.
pub(crate) fn failed(domain: &str) -> Result<(), CallError> {
    berm_lang::rows::<()>(
        "UPDATE crawl_sites SET failures = failures + 1 WHERE domain = ?",
        &(domain,),
    )?;
    Ok(())
}

/// Up to `limit` pages due, least recently fetched first: never fetched, their
/// URL's `lastmod` changed since, or with no `lastmod` and last fetched at
/// `before` or earlier.
pub(crate) fn pages_due(before: u64, limit: usize) -> Result<Vec<Due>, CallError> {
    Ok(berm_lang::rows::<(
        String,
        String,
        Option<String>,
        Option<String>,
        Option<String>,
    )>(
        "SELECT u.url, u.domain, u.lastmod, p.etag, p.modified
           FROM crawl_urls u LEFT JOIN crawl_pages p ON p.url = u.url
          WHERE p.url IS NULL
             OR (u.lastmod IS NOT NULL AND u.lastmod IS NOT p.lastmod)
             OR (u.lastmod IS NULL AND p.fetched <= ?)
          ORDER BY p.fetched NULLS FIRST LIMIT ?",
        &(before, limit as u64),
    )?
    .into_iter()
    .map(|(url, domain, lastmod, etag, modified)| Due {
        url,
        domain,
        lastmod,
        etag,
        modified,
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
        Fetched::Failed(_) => {
            berm_lang::rows::<()>(
                "INSERT INTO crawl_pages (url, status, lastmod, fetched) VALUES (?, 0, ?, ?)
                 ON CONFLICT (url) DO UPDATE SET status = 0, lastmod = excluded.lastmod,
                                                 fetched = excluded.fetched",
                &(&due.url, &due.lastmod, now),
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
    Ok(
        berm_lang::rows::<(String, Option<String>, Option<String>, u64, u64)>(
            "SELECT domain, robots, signal, read, failures FROM crawl_sites WHERE domain = ?",
            &(domain,),
        )?
        .pop()
        .map(|(domain, robots, signal, read, failures)| Watched {
            domain,
            robots,
            signal,
            read,
            failures,
        }),
    )
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
    )>(
        "SELECT url, status, fetched, title, description, text FROM crawl_pages WHERE url = ?",
        &(url,),
    )?
    .pop()
    .map(|(url, status, fetched, title, description, text)| Page {
        url,
        status,
        fetched,
        title,
        description,
        text,
    }))
}
