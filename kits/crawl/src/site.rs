//! One read of a site: its robots.txt, then its sitemaps.

use crate::{Crawler, fetch, robots::Robots, sitemap::Sitemap};
use alloc::{collections::VecDeque, format, string::String, vec::Vec};
use url::Url;

/// Sitemap files read per site.
const SITEMAPS: usize = 12;

/// The most of a robots.txt kept, in bytes.
const ROBOTS: usize = 64 << 10;

/// What a read found.
pub struct Site {
    /// Its robots.txt, up to 64 KiB; `None` when it answered 4xx.
    pub robots: Option<String>,
    pub signal: Option<String>,
    /// Each URL of a kind that robots.txt allows: the URL, its kind and its
    /// `lastmod` as written.
    pub urls: Vec<(String, &'static str, Option<String>)>,
}

/// Read `domain` as `C`. Fails when robots.txt cannot be read or answers 5xx;
/// a sitemap that fails is left out.
pub fn read<C: Crawler>(domain: &str) -> Result<Site, String> {
    let root =
        Url::parse(&format!("https://{domain}/")).map_err(|error| format!("{domain}: {error}"))?;
    let token = C::AGENT.split('/').next().unwrap_or(C::AGENT);
    let robots_url = root
        .join("/robots.txt")
        .map_err(|error| format!("{error}"))?;
    let answer = fetch::get(&robots_url, domain, C::AGENT, &[])?;
    let text = match answer.status {
        200..=299 => Some(String::from_utf8_lossy(&answer.body).into_owned()),
        400..=499 => None,
        status => return Err(format!("{robots_url} answered {status}")),
    };
    let robots = text
        .as_deref()
        .map(|text| Robots::parse(text, token))
        .unwrap_or_default();

    let mut queue: VecDeque<String> = robots.sitemaps.iter().cloned().collect();
    if queue.is_empty() {
        queue.push_back(format!("https://{domain}/sitemap.xml"));
    }
    let mut seen: Vec<String> = Vec::new();
    let mut urls = Vec::new();
    while let Some(next) = queue.pop_front() {
        if seen.len() == SITEMAPS {
            break;
        }
        if seen.contains(&next) || next.ends_with(".gz") {
            continue;
        }
        seen.push(next.clone());
        let Some(sitemap) = sitemap::<C>(&next, domain) else {
            continue;
        };
        queue.extend(sitemap.sitemaps);
        for (listed, lastmod) in sitemap.urls {
            let Ok(url) = Url::parse(&listed) else {
                continue;
            };
            if !url
                .host_str()
                .is_some_and(|host| fetch::same_site(host, domain))
            {
                continue;
            }
            let path = match url.query() {
                Some(query) => format!("{}?{query}", url.path()),
                None => String::from(url.path()),
            };
            if !robots.allows(&path) {
                continue;
            }
            if let Some(kind) = C::kind(domain, &url) {
                urls.push((String::from(url), kind, lastmod));
            }
        }
    }
    Ok(Site {
        robots: text.map(|mut text| {
            text.truncate(text.floor_char_boundary(ROBOTS));
            text
        }),
        signal: robots.signal,
        urls,
    })
}

/// The sitemap at `listed` on `domain`'s site, when it answers 200 with XML.
fn sitemap<C: Crawler>(listed: &str, domain: &str) -> Option<Sitemap> {
    let url = Url::parse(listed).ok()?;
    if !url
        .host_str()
        .is_some_and(|host| fetch::same_site(host, domain))
    {
        return None;
    }
    let answer = match fetch::get(&url, domain, C::AGENT, &[]) {
        Ok(answer) => answer,
        Err(error) => {
            berm_lang::trace(&error);
            return None;
        }
    };
    if answer.status != 200 {
        berm_lang::trace(&format!("{listed} answered {}", answer.status));
        return None;
    }
    Some(Sitemap::parse(&String::from_utf8_lossy(&answer.body)))
}
