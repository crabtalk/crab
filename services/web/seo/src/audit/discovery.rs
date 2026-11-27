use crate::{
    audit::{
        AGENT, MAX_CHECKED, MAX_URL_BYTES,
        markdown::{Coverage, Method},
        run::Run,
    },
    crawlers::{Robots, Rule},
};
use berm_lang::{Result, Schema, caller_storage, http, tool};
use sitemap::Sitemap;
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    ops::Bound::{Excluded, Included, Unbounded},
};
use url::{Position, Url};

const MAX_SITEMAPS: usize = 8;
const MAX_SITEMAP_BYTES: usize = 4 * 1024 * 1024;
const MAX_ROBOTS_BYTES: usize = 64 * 1024;
const HTTP_TIMEOUT: u64 = 2_500;
const LISTING_BATCH: usize = 500;

#[derive(Schema)]
pub(crate) struct CheckPlan {
    pub site: String,
    pub urls: Vec<String>,
}

pub(super) fn web_url(mut url: Url) -> Option<Url> {
    url.set_fragment(None);
    (matches!(url.scheme(), "http" | "https")
        && url.host_str().is_some()
        && url.username().is_empty()
        && url.password().is_none()
        && url.as_str().len() <= MAX_URL_BYTES)
        .then_some(url)
}

pub(super) fn request(url: &str, limit: usize) -> http::Request<'_> {
    http::Request {
        method: "GET",
        url,
        headers: &[("User-Agent", AGENT)],
        body: &[],
        timeout: Some(HTTP_TIMEOUT),
        limit: Some(limit),
    }
}

pub(super) fn allowed(robots: &Robots, url: &Url) -> bool {
    robots
        .crawler(AGENT, &url[Position::BeforePath..Position::AfterQuery])
        .rule
        != Rule::Blocked
}

pub(super) fn within(site: &Url, url: &Url) -> bool {
    site.origin() == url.origin()
        && (site.path() == url.path()
            || url
                .path()
                .strip_prefix(site.path())
                .is_some_and(|suffix| site.path().ends_with('/') || suffix.starts_with('/')))
}

fn robots(site: &Url) -> Result<Option<String>> {
    let url = site.join("/robots.txt").expect("site URL has an origin");
    Ok(
        http::fetch_all(&[request(url.as_str(), MAX_ROBOTS_BYTES + 1)])?
            .pop()
            .and_then(std::result::Result::ok)
            .and_then(|response| match response.status {
                200..300 if response.body.len() <= MAX_ROBOTS_BYTES => {
                    String::from_utf8(response.body).ok()
                }
                404 | 410 => Some(String::new()),
                _ => None,
            }),
    )
}

/// Discover sitemap pages
#[tool(
    access = "internal",
    budget(cpu = 100000, read = 1000000, written = 1000000, memory = 96)
)]
pub(crate) fn audit_discover(site: String) -> Result<CheckPlan> {
    let run = Run::owns(&site)?;
    let target = Url::parse(&site).map_err(|_| "Invalid audited site")?;
    let robots = robots(&target)?;
    let rules = Robots::from(robots.as_deref().unwrap_or_default());
    let mut queue: VecDeque<_> = rules.sitemaps.iter().cloned().collect();
    if queue.is_empty() {
        queue.push_back(
            target
                .join("/sitemap.xml")
                .expect("site has an origin")
                .into(),
        );
    }

    let mut visited = BTreeSet::new();
    let mut listed = BTreeMap::new();
    let mut complete = robots.is_some();
    while let Some(given) = queue.pop_front() {
        let Some(url) = target
            .join(&given)
            .ok()
            .and_then(web_url)
            .filter(|url| url.origin() == target.origin())
        else {
            complete = false;
            continue;
        };
        if visited.contains(&url) {
            continue;
        }
        if visited.len() == MAX_SITEMAPS {
            complete = false;
            break;
        }
        visited.insert(url.clone());
        let Some(text) = http::fetch_all(&[request(url.as_str(), MAX_SITEMAP_BYTES + 1)])?
            .pop()
            .and_then(std::result::Result::ok)
            .filter(|answer| answer.is_success() && answer.body.len() <= MAX_SITEMAP_BYTES)
            .and_then(|answer| String::from_utf8(answer.body).ok())
        else {
            complete = false;
            continue;
        };
        let sitemap = Sitemap::parse(&text);
        complete &= sitemap.complete;
        queue.extend(sitemap.sitemaps);
        for (given, _) in &sitemap.urls {
            let Some(url) = Url::parse(given).ok().and_then(web_url) else {
                complete = false;
                continue;
            };
            if within(&target, &url) {
                listed.insert(url.to_string(), robots.is_some() && !allowed(&rules, &url));
            }
        }
    }

    let urls: Vec<String> = if robots.is_some() {
        let cursor = caller_storage::rows::<(Option<String>,)>(
            "SELECT cursor FROM reports WHERE site = ?1",
            &(&site,),
        )?
        .pop()
        .and_then(|(cursor,)| cursor)
        .unwrap_or_default();
        listed
            .range::<str, _>((Excluded(cursor.as_str()), Unbounded))
            .chain(listed.range::<str, _>((Unbounded, Included(cursor.as_str()))))
            .take(MAX_CHECKED)
            .map(|(url, _)| url.clone())
            .collect()
    } else {
        Vec::new()
    };
    let root_markdown = if urls.is_empty() {
        0
    } else {
        let root = target.join("/").expect("site has an origin");
        match Method::Suffix.coverage(&[root], &rules, HTTP_TIMEOUT)?[0] {
            Coverage::Available(method) => method as u8,
            _ => 0,
        }
    };
    caller_storage::rows::<()>(
        "UPDATE runs SET robots = ?2, sitemap_complete = ?3, unlisted = NULL, root_markdown = ?5
         WHERE site = ?1 AND run = ?4",
        &(&site, robots.as_ref(), complete, &run.id, root_markdown),
    )?;
    reconcile(&site, listed, complete)?;
    Ok(CheckPlan { site, urls })
}

fn reconcile(site: &str, mut listed: BTreeMap<String, bool>, complete: bool) -> Result<()> {
    let mut after = String::new();
    let mut updates = Vec::new();
    loop {
        let rows = caller_storage::rows::<(String, u8)>(
            "SELECT url, blocked FROM pages WHERE site = ?1 AND url > ?2 ORDER BY url LIMIT ?3",
            &(site, &after, LISTING_BATCH),
        )?;
        let done = rows.len() < LISTING_BATCH;
        let mut missing = Vec::new();
        for (url, blocked) in rows {
            after.clone_from(&url);
            match listed.remove(&url) {
                Some(value) if value != (blocked != 0) => updates.push((url, value)),
                Some(_) => {}
                None => missing.push(url),
            }
        }
        if !missing.is_empty() {
            if complete {
                caller_storage::rows::<()>(
                    "DELETE FROM pages WHERE site = ?1 AND url IN (SELECT value FROM json_each(?2))",
                    &(site, &missing),
                )?;
            } else {
                caller_storage::rows::<()>(
                    "UPDATE runs SET unlisted = json_insert(coalesce(unlisted, '[]'), '$[#]', json(?2))
                     WHERE site = ?1",
                    &(site, &missing),
                )?;
            }
        }
        if done {
            break;
        }
    }
    updates.extend(listed);
    for batch in updates.chunks(LISTING_BATCH) {
        caller_storage::rows::<()>(
            "INSERT INTO pages (site, url, blocked)
             SELECT ?1, value ->> 0, value ->> 1 FROM json_each(?2) WHERE true
             ON CONFLICT (site, url) DO UPDATE SET blocked = excluded.blocked",
            &(site, batch),
        )?;
    }
    Ok(())
}
