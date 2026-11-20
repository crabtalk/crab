use crate::{
    audit::{AGENT, HTTP_TIMEOUT, MAX_PAGES, MAX_URL_BYTES},
    crawlers::{Robots, Rule},
};
use berm_lang::{Result, Schema, http, tool};
use sitemap::Sitemap;
use std::collections::{BTreeSet, VecDeque};
use url::{Position, Url};

const MAX_SITEMAPS: usize = 8;
const MAX_SITEMAP_BYTES: usize = 256 * 1024;
const MAX_ROBOTS_BYTES: usize = 64 * 1024;
const MAX_LISTED: usize = 1_000;

#[derive(Clone, Schema)]
pub(crate) struct Discovery {
    pub site: String,
    pub listed: Vec<String>,
    pub sample: Vec<Listed>,
    pub robots: String,
    pub sitemap_complete: bool,
    pub robots_read: bool,
}

#[derive(Clone, Schema)]
pub(crate) struct Listed {
    pub url: String,
    pub blocked: bool,
}

pub(crate) fn web_url(url: Url) -> Option<Url> {
    if matches!(url.scheme(), "http" | "https")
        && url.host_str().is_some()
        && url.username().is_empty()
        && url.password().is_none()
        && url.as_str().len() <= MAX_URL_BYTES
    {
        let mut url = url;
        url.set_fragment(None);
        Some(url)
    } else {
        None
    }
}

pub(crate) fn request(url: &str, limit: usize) -> http::Request<'_> {
    http::Request {
        method: "GET",
        url,
        headers: &[("User-Agent", AGENT)],
        body: &[],
        timeout: Some(HTTP_TIMEOUT),
        limit: Some(limit),
    }
}

pub(crate) fn allowed(robots: &Robots, url: &Url) -> bool {
    robots
        .crawler(AGENT, &url[Position::BeforePath..Position::AfterQuery])
        .rule
        != Rule::Blocked
}

pub(crate) fn within(site: &Url, url: &Url) -> bool {
    site.origin() == url.origin()
        && (site.path() == url.path()
            || url
                .path()
                .strip_prefix(site.path())
                .is_some_and(|suffix| site.path().ends_with('/') || suffix.starts_with('/')))
}

/// Discover sitemap pages
///
/// Reads robots.txt and a bounded set of sitemap files on the site's origin.
#[tool(
    query,
    access = "internal",
    budget(cpu = 100000, read = 1, written = 1, memory = 32)
)]
pub(crate) fn audit_discover(site: String) -> Result<Discovery> {
    let mut site = Url::parse(&site)
        .ok()
        .and_then(web_url)
        .ok_or("Provide a public HTTP or HTTPS site URL of at most 512 bytes")?;
    site.set_query(None);
    let root = |path: &str| site.join(path).expect("site URL has an origin");
    let robots_url = root("/robots.txt");
    let robots_response = http::fetch_all(&[request(robots_url.as_str(), MAX_ROBOTS_BYTES + 1)])?
        .pop()
        .and_then(std::result::Result::ok);
    let (robots, robots_read) = match robots_response {
        Some(response) if response.is_success() && response.body.len() <= MAX_ROBOTS_BYTES => {
            match String::from_utf8(response.body) {
                Ok(text) => (text, true),
                Err(_) => (String::new(), false),
            }
        }
        Some(response) if matches!(response.status, 404 | 410) => (String::new(), true),
        _ => (String::new(), false),
    };
    let rules = Robots::from(robots.as_str());
    let mut queue: VecDeque<_> = rules.sitemaps.iter().cloned().collect();
    if queue.is_empty() {
        queue.push_back(root("/sitemap.xml").to_string());
    }

    let mut visited = BTreeSet::new();
    let mut listed = BTreeSet::new();
    let mut complete = true;
    let mut read = false;
    while !queue.is_empty() {
        let mut batch = Vec::new();
        while let Some(given) = queue.pop_front() {
            let Some(url) = site
                .join(&given)
                .ok()
                .and_then(web_url)
                .filter(|url| url.origin() == site.origin())
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
            batch.push(url);
        }
        let requests: Vec<_> = batch
            .iter()
            .map(|url| request(url.as_str(), MAX_SITEMAP_BYTES + 1))
            .collect();
        if requests.is_empty() {
            break;
        }
        for response in http::fetch_all(&requests)? {
            let Some(text) = response
                .ok()
                .filter(|answer| answer.is_success() && answer.body.len() <= MAX_SITEMAP_BYTES)
                .and_then(|answer| String::from_utf8(answer.body).ok())
            else {
                complete = false;
                continue;
            };
            let sitemap = Sitemap::parse(&text);
            read |= sitemap.complete;
            complete &= sitemap.complete;
            queue.extend(sitemap.sitemaps);
            for (url, _) in sitemap.urls {
                let Some(url) = Url::parse(&url).ok().and_then(web_url) else {
                    complete = false;
                    continue;
                };
                if !within(&site, &url) || listed.contains(&url) {
                    continue;
                }
                if listed.len() == MAX_LISTED {
                    complete = false;
                    break;
                }
                listed.insert(url);
            }
        }
        if visited.len() == MAX_SITEMAPS && !queue.is_empty() {
            complete = false;
            break;
        }
    }
    if !read {
        return Err("No readable sitemap was found on the site's origin".into());
    }

    let sample = listed
        .iter()
        .take(MAX_PAGES)
        .map(|url| Listed {
            url: url.to_string(),
            blocked: !robots_read || !allowed(&rules, url),
        })
        .collect();
    Ok(Discovery {
        site: site.to_string(),
        listed: listed.into_iter().map(Into::into).collect(),
        sample,
        robots,
        sitemap_complete: complete,
        robots_read,
    })
}
