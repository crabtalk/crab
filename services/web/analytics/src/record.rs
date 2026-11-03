use crate::{
    agent, site,
    types::{AnalyticsError, Recorded},
};
use alloc::string::String;
use berm_lang::Result;

const MAX_EVENTS: u64 = 50_000;
const MAX_DIMENSIONS: u64 = 50_000;

/// Records one view of `page` on `site`. The site's script sends each page
/// a person opens, without `agent`. Its crawler report sends every request
/// with the request's user agent, and only a request from an AI crawler is
/// kept. A person's
/// view counts its client as a visitor when the host names the client.
pub fn record(
    site: String,
    page: String,
    referrer: Option<String>,
    agent: Option<String>,
) -> Result<Recorded> {
    if referrer.as_ref().is_some_and(|value| value.len() > 4096)
        || agent.as_ref().is_some_and(|value| value.len() > 1024)
    {
        return Err("Referrer exceeds 4096 bytes or user agent exceeds 1024 bytes".into());
    }
    let Some((domain,)) =
        berm_lang::rows::<(String,)>("SELECT domain FROM sites WHERE id = ?", &(&site,))?.pop()
    else {
        return Err(AnalyticsError::NoSite.into());
    };
    let page = page.split(['?', '#']).next().unwrap_or_default();
    if !page.starts_with('/') || page.len() > site::MAX_PAGE || page.chars().any(char::is_control) {
        return Err(AnalyticsError::BadPage.into());
    }
    let crawler = match agent.as_deref() {
        Some(agent) => match agent::crawler(agent) {
            Some(crawler) => crawler,
            None => return Ok(Recorded { kept: false }),
        },
        None => "",
    };
    let source = referrer
        .as_deref()
        .and_then(site::host)
        .filter(|host| !site::same_site(host, &domain))
        .unwrap_or_default();
    let (count,) = berm_lang::rows::<(u64,)>("SELECT COUNT(*) FROM events", &[0u8; 0])?
        .pop()
        .unwrap_or_default();
    if count >= MAX_EVENTS {
        return Err(
            "This copy holds 50000 raw events; prune or remove a site before recording more".into(),
        );
    }
    let at = berm_lang::now();
    let day = at / site::DAY;
    let held = berm_lang::rows::<(u64,)>(
        "SELECT 1 FROM days WHERE site = ? AND day = ? AND page = ? AND source = ? AND crawler = ?",
        &(&site, day, page, &source, crawler),
    )?;
    if held.is_empty() {
        let (count,) = berm_lang::rows::<(u64,)>("SELECT COUNT(*) FROM days", &[0u8; 0])?
            .pop()
            .unwrap_or_default();
        if count >= MAX_DIMENSIONS {
            return Err(
                "This copy holds 50000 daily dimensions; prune or remove a site before adding more"
                    .into(),
            );
        }
    }
    berm_lang::rows::<()>(
        "INSERT INTO events (site, at, page, source, crawler) VALUES (?, ?, ?, ?, ?)",
        &(&site, at, page, &source, crawler),
    )?;
    berm_lang::rows::<()>(
        "INSERT INTO days (site, day, page, source, crawler, views) VALUES (?, ?, ?, ?, ?, 1)
         ON CONFLICT (site, day, page, source, crawler) DO UPDATE SET views = views + 1",
        &(&site, day, page, &source, crawler),
    )?;
    if crawler.is_empty()
        && let Some(key) = berm_lang::client()?
    {
        let (seen_day, seen_page, seen_source) = berm_lang::rows::<(u8, u8, u8)>(
            "SELECT EXISTS (SELECT 1 FROM seen WHERE site = ?1 AND day = ?2 AND key = ?3),
                    EXISTS (SELECT 1 FROM seen
                             WHERE site = ?1 AND day = ?2 AND key = ?3 AND page = ?4),
                    EXISTS (SELECT 1 FROM seen
                             WHERE site = ?1 AND day = ?2 AND key = ?3 AND source = ?5)",
            &(&site, day, &key, page, &source),
        )?
        .pop()
        .unwrap_or_default();
        berm_lang::rows::<()>(
            "INSERT OR IGNORE INTO seen (site, day, key, page, source) VALUES (?, ?, ?, ?, ?)",
            &(&site, day, &key, page, &source),
        )?;
        if seen_day == 0 {
            berm_lang::rows::<()>(
                "INSERT INTO visitors (site, day, visitors) VALUES (?, ?, 1)
                 ON CONFLICT (site, day) DO UPDATE SET visitors = visitors + 1",
                &(&site, day),
            )?;
        }
        if seen_page == 0 {
            berm_lang::rows::<()>(
                "INSERT INTO pages (site, day, page, visitors) VALUES (?, ?, ?, 1)
                 ON CONFLICT (site, day, page) DO UPDATE SET visitors = visitors + 1",
                &(&site, day, page),
            )?;
        }
        if seen_source == 0 && !source.is_empty() {
            berm_lang::rows::<()>(
                "INSERT INTO sources (site, day, source, visitors) VALUES (?, ?, ?, 1)
                 ON CONFLICT (site, day, source) DO UPDATE SET visitors = visitors + 1",
                &(&site, day, &source),
            )?;
        }
    }
    Ok(Recorded { kept: true })
}

/// Deletes views recorded more than 30 days ago, and the client keys of
/// days that are over. Daily rollups are retained for 366 days.
/// Deletes at most 1000 expired rows per table in one call.
pub fn prune() -> Result<()> {
    let now = berm_lang::now();
    berm_lang::rows::<()>(
        "DELETE FROM events WHERE seq IN (SELECT seq FROM events WHERE at < ? ORDER BY at LIMIT 1000)",
        &(now.saturating_sub(site::KEPT),),
    )?;
    berm_lang::rows::<()>(
        "DELETE FROM seen WHERE rowid IN (SELECT rowid FROM seen WHERE day < ? LIMIT 1000)",
        &(now / site::DAY,),
    )?;
    let oldest = (now / site::DAY).saturating_sub(365);
    for table in ["days", "visitors", "pages", "sources"] {
        berm_lang::rows::<()>(
            &alloc::format!(
                "DELETE FROM {table} WHERE rowid IN (SELECT rowid FROM {table} WHERE day < ? LIMIT 1000)"
            ),
            &(oldest,),
        )?;
    }
    Ok(())
}
