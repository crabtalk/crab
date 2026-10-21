//! The reads of a tick's sites: each one's robots.txt, then its sitemaps, the
//! sites read at once, a request a site a round.

use crate::{
    Crawler,
    fetch::{self, Get},
    robots::Robots,
    sitemap::Sitemap,
};
use alloc::{collections::VecDeque, format, string::String, vec::Vec};
use berm_lang::http::Response;
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

/// A site being read.
struct Reading<'a> {
    domain: &'a str,
    robots: Robots,
    text: Option<String>,
    queue: VecDeque<String>,
    seen: Vec<String>,
    urls: Vec<(String, &'static str, Option<String>)>,
    /// What the read came to, once it did.
    done: Option<Result<Option<Site>, String>>,
}

/// Read each of `domains` as `C`, at once, and answer each in its place. The
/// first is read for `budget` sitemap URLs of its own; the rest share another
/// `budget`, and one that would list more than are left answers `None`, left
/// for a later tick. A read fails when robots.txt cannot be read or answers
/// 5xx; a sitemap that fails is left out.
pub fn read_all<C: Crawler>(
    domains: &[String],
    budget: usize,
) -> Vec<Result<Option<Site>, String>> {
    let token = C::AGENT.split('/').next().unwrap_or(C::AGENT);
    let timeout = C::SCHEDULE.timeout;
    let mut readings: Vec<Reading<'_>> = domains
        .iter()
        .map(|domain| Reading {
            domain,
            robots: Robots::default(),
            text: None,
            queue: VecDeque::new(),
            seen: Vec::new(),
            urls: Vec::new(),
            done: None,
        })
        .collect();

    let mut round: Vec<usize> = Vec::new();
    let mut gets: Vec<Get<'_>> = Vec::new();
    for (at, reading) in readings.iter_mut().enumerate() {
        match Url::parse(&format!("https://{}/robots.txt", reading.domain)) {
            Ok(url) => {
                round.push(at);
                gets.push(Get {
                    url,
                    domain: reading.domain,
                    headers: Vec::new(),
                });
            }
            Err(error) => reading.done = Some(Err(format!("{}: {error}", reading.domain))),
        }
    }
    let answers = fetch::get_all(&gets, C::AGENT, timeout);
    for ((at, get), answer) in round.into_iter().zip(&gets).zip(answers) {
        let reading = &mut readings[at];
        let answer = match answer {
            Ok(answer) => answer,
            Err(error) => {
                reading.done = Some(Err(error));
                continue;
            }
        };
        reading.text = match answer.status {
            200..=299 => Some(String::from_utf8_lossy(&answer.body).into_owned()),
            400..=499 => None,
            status => {
                reading.done = Some(Err(format!("{} answered {status}", get.url)));
                continue;
            }
        };
        reading.robots = reading
            .text
            .as_deref()
            .map(|text| Robots::parse(text, token))
            .unwrap_or_default();
        reading.queue = reading.robots.sitemaps.iter().cloned().collect();
        if reading.queue.is_empty() {
            reading
                .queue
                .push_back(format!("https://{}/sitemap.xml", reading.domain));
        }
    }

    // What the first site may still list, and what the rest may.
    let (mut own, mut shared) = (budget, budget);
    loop {
        let mut round: Vec<usize> = Vec::new();
        let mut gets: Vec<Get<'_>> = Vec::new();
        for (at, reading) in readings.iter_mut().enumerate() {
            if reading.done.is_some() {
                continue;
            }
            match next(reading) {
                Some(url) => {
                    round.push(at);
                    gets.push(Get {
                        url,
                        domain: reading.domain,
                        headers: Vec::new(),
                    });
                }
                None => reading.done = Some(Ok(Some(finish(reading)))),
            }
        }
        if round.is_empty() {
            break;
        }
        let answers = fetch::get_all(&gets, C::AGENT, timeout);
        for ((at, get), answer) in round.into_iter().zip(&gets).zip(answers) {
            let reading = &mut readings[at];
            if reading.done.is_some() {
                continue;
            }
            let left = match at {
                0 => &mut own,
                _ => &mut shared,
            };
            let Some(sitemap) = sitemap(get, answer, *left) else {
                continue;
            };
            if sitemap.more && at > 0 {
                reading.done = Some(Ok(None));
                continue;
            }
            reading.queue.extend(sitemap.sitemaps);
            *left -= sitemap.urls.len();
            let full = sitemap.more || *left == 0;
            keep::<C>(reading, sitemap.urls);
            if full {
                reading.done = Some(Ok(Some(finish(reading))));
            }
        }
    }
    readings
        .into_iter()
        .map(|reading| reading.done.unwrap_or(Ok(None)))
        .collect()
}

/// The next sitemap of `reading`'s to fetch, on its site; `None` when it has
/// none left or has read [`SITEMAPS`].
fn next(reading: &mut Reading<'_>) -> Option<Url> {
    while let Some(listed) = reading.queue.pop_front() {
        if reading.seen.len() == SITEMAPS {
            return None;
        }
        if reading.seen.contains(&listed) || listed.ends_with(".gz") {
            continue;
        }
        reading.seen.push(listed.clone());
        let Ok(url) = Url::parse(&listed) else {
            continue;
        };
        if url
            .host_str()
            .is_some_and(|host| crate::same_site(host, reading.domain))
        {
            return Some(url);
        }
    }
    None
}

/// The sitemap `answer` holds, its first `limit` URLs, when it answered 200.
fn sitemap(get: &Get<'_>, answer: Result<Response, String>, limit: usize) -> Option<Sitemap> {
    let answer = match answer {
        Ok(answer) => answer,
        Err(error) => {
            berm_lang::trace(&error);
            return None;
        }
    };
    if answer.status != 200 {
        berm_lang::trace(&format!("{} answered {}", get.url, answer.status));
        return None;
    }
    Some(Sitemap::parse(
        &String::from_utf8_lossy(&answer.body),
        limit,
    ))
}

/// Keep each of `listed` on `reading`'s site that its robots.txt allows and
/// that is of a kind `C` names.
fn keep<C: Crawler>(reading: &mut Reading<'_>, listed: Vec<(String, Option<String>)>) {
    for (loc, lastmod) in listed {
        let Ok(url) = Url::parse(&loc) else {
            continue;
        };
        if !url
            .host_str()
            .is_some_and(|host| crate::same_site(host, reading.domain))
        {
            continue;
        }
        let path = match url.query() {
            Some(query) => format!("{}?{query}", url.path()),
            None => String::from(url.path()),
        };
        if !reading.robots.allows(&path) {
            continue;
        }
        if let Some(kind) = C::kind(reading.domain, &url) {
            reading.urls.push((String::from(url), kind, lastmod));
        }
    }
}

/// What `reading` read.
fn finish(reading: &mut Reading<'_>) -> Site {
    Site {
        robots: reading.text.take().map(|mut text| {
            text.truncate(text.floor_char_boundary(ROBOTS));
            text
        }),
        signal: reading.robots.signal.take(),
        urls: core::mem::take(&mut reading.urls),
    }
}
