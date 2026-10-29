//! What a crawler read, in its database: `crawl_sites`, `crawl_urls` and
//! `crawl_pages`.

use crate::{page::Fetched, site::Site};
use alloc::{string::String, vec::Vec};
use berm_lang::{CallError, Schema, serde::Deserialize};
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
#[derive(Deserialize)]
#[serde(crate = "berm_lang::serde")]
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

/// Watch `domains`, each lowercased, read before the sites not `first` when
/// `first`. Answers how many were new.
pub(crate) fn watch(domains: &[String], first: bool) -> Result<usize, CallError> {
    let domains: Vec<String> = domains
        .iter()
        .map(|domain| domain.to_ascii_lowercase())
        .collect();
    let new = berm_lang::rows::<(u8,)>(
        "INSERT INTO crawl_sites (domain, first) SELECT value, ?2 FROM json_each(?1) WHERE true
         ON CONFLICT (domain) DO NOTHING RETURNING 1",
        &(&domains, u8::from(first)),
    )?
    .len();
    if first {
        berm_lang::rows::<()>(
            "UPDATE crawl_sites SET first = 1 WHERE domain IN (SELECT value FROM json_each(?))",
            &(&domains,),
        )?;
    }
    Ok(new)
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
/// the last, or the last kept with the sitemaps' failure as its error, and
/// only the URLs its robots.txt still allows. Answers the bytes written.
pub(crate) fn read(domain: &str, now: u64, site: &Site) -> Result<usize, CallError> {
    let last = robots(domain)?;
    berm_lang::rows::<()>(
        "UPDATE crawl_sites SET robots = ?, signal = ?, read = ?, failures = 0, error = ?
          WHERE domain = ?",
        &(
            &site.robots,
            &site.rules.signal,
            now,
            site.urls.as_ref().err(),
            domain,
        ),
    )?;
    if let Ok(urls) = &site.urls {
        berm_lang::rows::<()>(
            "DELETE FROM crawl_urls WHERE domain = ? AND listed = 0",
            &(domain,),
        )?;
        berm_lang::rows::<()>(
            "INSERT INTO crawl_urls (url, domain, kind, lastmod, due)
             SELECT j.value ->> 0, ?1, j.value ->> 1, j.value ->> 2,
                    CASE WHEN p.url IS NULL THEN 0
                         WHEN j.value ->> 2 IS NULL THEN p.fetched
                         WHEN j.value ->> 2 IS NOT p.lastmod THEN 0 END
               FROM json_each(?2) AS j LEFT JOIN crawl_pages p ON p.url = j.value ->> 0
              WHERE true
             ON CONFLICT (url) DO UPDATE SET kind = excluded.kind, lastmod = excluded.lastmod,
                                             due = excluded.due",
            &(domain, urls),
        )?;
    }
    // A sitemap's URLs were checked against this robots.txt as read, and the
    // rest against `last` as stored, which reads as this one when whole.
    if last != site.robots || !site.whole() {
        let statement = match site.urls {
            Ok(_) => "SELECT url FROM crawl_urls WHERE domain = ? AND listed = 1",
            Err(_) => "SELECT url FROM crawl_urls WHERE domain = ?",
        };
        let disallowed: Vec<String> = berm_lang::rows::<(String,)>(statement, &(domain,))?
            .into_iter()
            .filter_map(|(url,)| {
                Url::parse(&url)
                    .is_ok_and(|parsed| !site.rules.allows(&parsed))
                    .then_some(url)
            })
            .collect();
        berm_lang::rows::<()>(
            "DELETE FROM crawl_urls WHERE url IN (SELECT value FROM json_each(?))",
            &(&disallowed,),
        )?;
    }
    let urls: usize = site
        .urls
        .iter()
        .flatten()
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

/// Keep each of `urls`, a URL and its kind, as listed for `domain`, a site
/// watched, read for `bytes` of HTML when given. Answers how many were new.
pub(crate) fn list(
    domain: &str,
    urls: &[(String, &str)],
    bytes: Option<usize>,
) -> Result<usize, CallError> {
    let bytes = bytes.map(|bytes| bytes as u64);
    let new = berm_lang::rows::<(u8,)>(
        "INSERT INTO crawl_urls (url, domain, kind, listed, bytes, due)
         SELECT j.value ->> 0, ?1, j.value ->> 1, 1, ?3, coalesce(p.fetched, 0)
           FROM json_each(?2) AS j LEFT JOIN crawl_pages p ON p.url = j.value ->> 0
          WHERE true
         ON CONFLICT (url) DO NOTHING RETURNING 1",
        &(domain, urls, bytes),
    )?
    .len();
    berm_lang::rows::<()>(
        "UPDATE crawl_urls SET listed = 1, bytes = ?1
          WHERE url IN (SELECT value ->> 0 FROM json_each(?2))",
        &(bytes, urls),
    )?;
    Ok(new)
}

/// Up to `limit` pages due on sites read, those of sites watched first before
/// the rest, a site's soonest due before any site's next: never fetched or
/// their URL's `lastmod` changed since, then those with no `lastmod` last
/// fetched at `before` or earlier, least recently fetched first.
pub(crate) fn pages_due(before: u64, limit: usize) -> Result<Vec<Due>, CallError> {
    berm_lang::rows::<Due>(
        "SELECT u.url, u.domain, u.kind, u.lastmod, p.etag, p.modified, u.bytes
           FROM crawl_sites s
                JOIN crawl_urls u ON u.url IN (SELECT url FROM crawl_urls
                                                WHERE domain = s.domain AND due <= ?1
                                                ORDER BY due LIMIT ?2)
                LEFT JOIN crawl_pages p ON p.url = u.url
          WHERE s.read > 0
          ORDER BY s.first DESC,
                   row_number() OVER (PARTITION BY u.domain ORDER BY u.due),
                   u.due
          LIMIT ?2",
        &(before, limit as u64),
    )
}

/// Store what fetching `due` at `now` found. Answers the bytes written.
pub(crate) fn fetched(due: &Due, now: u64, fetched: &Fetched) -> Result<usize, CallError> {
    berm_lang::rows::<()>(
        "UPDATE crawl_urls SET due = CASE WHEN lastmod IS NULL THEN ?1
                                          WHEN lastmod IS NOT ?2 THEN 0 END
          WHERE url = ?3",
        &(now, &due.lastmod, &due.url),
    )?;
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
    Ok(berm_lang::rows::<Watched>(
        "SELECT domain, robots, signal, read, failures, error, failed FROM crawl_sites
          WHERE domain = ?",
        &(domain,),
    )?
    .pop())
}

/// Up to `limit` sites whose last read failed at `since` or later, most
/// recently failed first.
pub fn failed(since: u64, limit: usize) -> Result<Vec<Watched>, CallError> {
    berm_lang::rows::<Watched>(
        "SELECT domain, robots, signal, read, failures, error, failed FROM crawl_sites
          WHERE failures > 0 AND failed >= ? ORDER BY failed DESC LIMIT ?",
        &(since, limit as u64),
    )
}

/// The URLs `domain`'s sitemaps list, of `kind` when given, in URL order.
pub fn urls(domain: &str, kind: Option<&str>) -> Result<Vec<Listed>, CallError> {
    berm_lang::rows::<Listed>(
        "SELECT url, kind, lastmod FROM crawl_urls
          WHERE domain = ? AND (? IS NULL OR kind = ?) ORDER BY url",
        &(domain, kind, kind),
    )
}

/// The page at `url` as last fetched.
pub fn page(url: &str) -> Result<Option<Page>, CallError> {
    Ok(berm_lang::rows::<Page>(
        "SELECT url, status, fetched, title, description, text, error FROM crawl_pages
          WHERE url = ?",
        &(url,),
    )?
    .pop())
}
