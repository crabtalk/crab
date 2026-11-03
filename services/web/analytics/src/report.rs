use crate::{
    agent,
    site::{self, Site},
    types::{
        AnalyticsError, Crawl, Crawlers, Day, Days, Event, Events, Page, Pages, Source, Sources,
    },
};
use alloc::{string::String, vec::Vec};
use berm_lang::Result;

/// Counts each day's views of one of the deployment's sites, from
/// `from` to `to`: people's views, those referred by an AI assistant, and
/// AI crawls. Days with none are left out.
pub fn days(site: String, from: Option<u64>, to: Option<u64>) -> Result<Days> {
    Site::get(&site)?.ok_or(AnalyticsError::NoSite)?;
    let (first, last) = site::span(from, to)?;
    let found = berm_lang::rows::<(u64, u64, u64, u64, u64)>(
        "SELECT d.day,
                SUM(CASE WHEN d.crawler = '' THEN d.views ELSE 0 END),
                COALESCE(v.visitors, 0),
                SUM(CASE WHEN d.crawler = '' AND d.source IN (SELECT value FROM json_each(?4)) THEN d.views ELSE 0 END),
                SUM(CASE WHEN d.crawler != '' THEN d.views ELSE 0 END)
           FROM days d LEFT JOIN visitors v ON v.site = d.site AND v.day = d.day
          WHERE d.site = ?1 AND d.day BETWEEN ?2 AND ?3 GROUP BY d.day ORDER BY d.day",
        &(&site, first, last, agent::ASSISTANTS),
    )?;
    Ok(Days {
        days: found
            .into_iter()
            .map(|(day, views, visitors, referred, crawls)| Day {
                day: day * site::DAY,
                views,
                visitors,
                referred,
                crawls,
            })
            .collect(),
    })
}

/// Ranks the pages of one of the deployment's sites by people's
/// views from `from` to `to`.
pub fn pages(
    site: String,
    from: Option<u64>,
    to: Option<u64>,
    limit: Option<u64>,
) -> Result<Pages> {
    Site::get(&site)?.ok_or(AnalyticsError::NoSite)?;
    let (first, last) = site::span(from, to)?;
    let found = berm_lang::rows::<(String, u64, u64)>(
        "WITH totals AS (
            SELECT page, SUM(views) AS views FROM days
             WHERE site = ?1 AND day BETWEEN ?2 AND ?3 AND crawler = '' GROUP BY page ORDER BY views DESC, page LIMIT ?4
         ), people AS (
            SELECT page, SUM(visitors) AS visitors FROM pages WHERE site = ?1 AND day BETWEEN ?2 AND ?3 GROUP BY page
         ) SELECT t.page, t.views, COALESCE(p.visitors, 0) FROM totals t LEFT JOIN people p ON p.page = t.page ORDER BY t.views DESC, t.page",
        &(&site, first, last, site::limit(limit)),
    )?;
    let pages = found
        .into_iter()
        .map(|(page, views, visitors)| Page {
            page,
            views,
            visitors,
        })
        .collect();
    Ok(Pages { pages })
}

/// Ranks the hosts that referred people to one of the deployment's
/// sites by views from `from` to `to`. Views with no referrer, or referred
/// from the site itself, are left out.
pub fn sources(
    site: String,
    from: Option<u64>,
    to: Option<u64>,
    limit: Option<u64>,
) -> Result<Sources> {
    Site::get(&site)?.ok_or(AnalyticsError::NoSite)?;
    let (first, last) = site::span(from, to)?;
    let found = berm_lang::rows::<(String, u64, u64)>(
        "WITH totals AS (
            SELECT source, SUM(views) AS views FROM days
             WHERE site = ?1 AND day BETWEEN ?2 AND ?3 AND crawler = '' AND source != '' GROUP BY source ORDER BY views DESC, source LIMIT ?4
         ), people AS (
            SELECT source, SUM(visitors) AS visitors FROM sources WHERE site = ?1 AND day BETWEEN ?2 AND ?3 GROUP BY source
         ) SELECT t.source, t.views, COALESCE(p.visitors, 0) FROM totals t LEFT JOIN people p ON p.source = t.source ORDER BY t.views DESC, t.source",
        &(&site, first, last, site::limit(limit)),
    )?;
    let sources = found
        .into_iter()
        .map(|(host, views, visitors)| Source {
            assistant: agent::assistant(&host),
            host,
            views,
            visitors,
        })
        .collect();
    Ok(Sources { sources })
}

/// Ranks the AI crawlers that fetched one of the deployment's sites
/// by requests from `from` to `to`.
pub fn crawlers(site: String, from: Option<u64>, to: Option<u64>) -> Result<Crawlers> {
    Site::get(&site)?.ok_or(AnalyticsError::NoSite)?;
    let (first, last) = site::span(from, to)?;
    let found = berm_lang::rows::<(String, u64)>(
        "SELECT crawler, SUM(views) AS total FROM days
          WHERE site = ? AND day BETWEEN ? AND ? AND crawler != ''
          GROUP BY crawler ORDER BY total DESC, crawler",
        &(&site, first, last),
    )?;
    let crawlers = found
        .into_iter()
        .map(|(crawler, requests)| Crawl { crawler, requests })
        .collect();
    Ok(Crawlers { crawlers })
}

/// Lists the views recorded for one of the deployment's sites after
/// `after`, oldest first. Views are kept 30 days.
pub fn events(site: String, after: Option<u64>, limit: Option<u64>) -> Result<Events> {
    Site::get(&site)?.ok_or(AnalyticsError::NoSite)?;
    let after = after.unwrap_or(0);
    let found = berm_lang::rows::<(u64, u64, String, String, String)>(
        "SELECT seq, at, page, source, crawler FROM events
          WHERE site = ? AND seq > ? ORDER BY seq LIMIT ?",
        &(&site, after, site::limit(limit)),
    )?;
    let events: Vec<Event> = found
        .into_iter()
        .map(|(seq, at, page, source, crawler)| Event {
            seq,
            at,
            page,
            source: Some(source).filter(|source| !source.is_empty()),
            crawler: Some(crawler).filter(|crawler| !crawler.is_empty()),
        })
        .collect();
    Ok(Events {
        next: events.last().map_or(after, |event| event.seq),
        events,
    })
}
