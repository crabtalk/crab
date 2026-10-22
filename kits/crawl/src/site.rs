//! The reads of a tick's sites: each one's robots.txt, then its sitemaps, the
//! sites read at once, a request a site a round.

use crate::{
    Crawler,
    fetch::{self, Get},
    robots::Robots,
    sitemap::Sitemap,
};
use alloc::{collections::VecDeque, format, string::String, vec::Vec};
use core::mem;
use url::Url;

/// Sitemap files read per site.
const SITEMAPS: usize = 12;

/// The most of a robots.txt kept, in bytes.
const ROBOTS: usize = 64 << 10;

/// The most of a robots.txt read, in bytes: the least RFC 9309 has a crawler
/// parse.
const ROBOTS_READ: usize = 500 << 10;

/// A URL a sitemap listed, of a kind that robots.txt allows: the URL, its kind
/// and its `lastmod` as written.
pub type Listed = (String, &'static str, Option<String>);

/// What a read found.
pub struct Site {
    /// Its robots.txt, cut at [`ROBOTS`] bytes; `None` when it answered 4xx.
    pub robots: Option<String>,
    /// What the crawler reads out of its robots.txt.
    pub rules: Robots,
    /// Each URL its sitemaps listed; the first sitemap's failure when one
    /// timed out or answered 5xx and none answered that names no sitemaps or
    /// lists URLs.
    pub urls: Result<Vec<Listed>, String>,
}

/// A site being read.
struct Reading<'a> {
    domain: &'a str,
    robots: Robots,
    text: Option<String>,
    queue: VecDeque<String>,
    seen: Vec<String>,
    urls: Vec<Listed>,
    /// Whether a sitemap answered that names no sitemaps or lists URLs.
    answered: bool,
    /// Why the first sitemap that failed failed.
    failed: Option<String>,
    /// What the read came to, once it did.
    done: Option<Result<Option<Site>, String>>,
}

/// Read each of `domains` as `C`, at once, and answer each in its place. The
/// first is read for `budget` bytes of sitemaps of its own; the rest share
/// another `budget`, each round's split evenly among those reading, and one
/// whose sitemap is cut at its share answers `None`, left for a later tick. A
/// read fails when robots.txt cannot be read or answers 5xx. A sitemap
/// answering anything but 200, a timeout or 5xx, is absent.
pub fn read_all<C: Crawler>(
    domains: &[String],
    budget: usize,
) -> Vec<Result<Option<Site>, String>> {
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
            answered: false,
            failed: None,
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
                    limit: ROBOTS_READ,
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
            200..=299 => Some(
                String::from_utf8(answer.body)
                    .unwrap_or_else(|error| String::from_utf8_lossy(error.as_bytes()).into_owned()),
            ),
            400..=499 => None,
            status => {
                reading.done = Some(Err(format!("{} answered {status}", get.url)));
                continue;
            }
        };
        reading.robots = reading
            .text
            .as_deref()
            .map(|text| Robots::parse(text, C::AGENT))
            .unwrap_or_default();
        reading.queue = reading.robots.sitemaps.iter().cloned().collect();
        if reading.queue.is_empty() {
            reading
                .queue
                .push_back(format!("https://{}/sitemap.xml", reading.domain));
        }
    }

    // The bytes of sitemaps the first site may still read, and the rest may.
    let (mut own, mut shared) = (budget, budget);
    loop {
        let mut next_urls: Vec<(usize, Url)> = Vec::new();
        for (at, reading) in readings.iter_mut().enumerate() {
            if reading.done.is_some() {
                continue;
            }
            match next(reading) {
                Some(url) => next_urls.push((at, url)),
                None => reading.done = Some(finish(reading).map(Some)),
            }
        }
        // Each site after the first is read for an even share of `shared`: a
        // round's answers to them never take more than it.
        let others = next_urls.iter().filter(|(at, _)| *at > 0).count();
        let share = shared / others.max(1);
        let mut round: Vec<usize> = Vec::new();
        let mut gets: Vec<Get<'_>> = Vec::new();
        for (at, url) in next_urls {
            let reading = &mut readings[at];
            let left = if at == 0 { own } else { share };
            match left {
                0 if at > 0 => reading.done = Some(Ok(None)),
                0 => reading.done = Some(finish(reading).map(Some)),
                _ => {
                    round.push(at);
                    gets.push(Get {
                        url,
                        domain: reading.domain,
                        headers: Vec::new(),
                        limit: left,
                    });
                }
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
            let body = match answer {
                Ok(answer) if answer.status == 200 => answer.body,
                Ok(answer) if !(500..=599).contains(&answer.status) => continue,
                answer => {
                    let error = match answer {
                        Ok(answer) => format!("{} answered {}", get.url, answer.status),
                        Err(error) => error,
                    };
                    berm_lang::trace(&error);
                    reading.failed.get_or_insert(error);
                    continue;
                }
            };
            // An answer as long as its limit was cut there.
            let cut = body.len() >= get.limit;
            if at > 0 && cut {
                reading.done = Some(Ok(None));
                continue;
            }
            *left = left.saturating_sub(body.len());
            let sitemap = Sitemap::parse(&String::from_utf8_lossy(&body));
            reading.answered |= sitemap.sitemaps.is_empty() || !sitemap.urls.is_empty();
            reading.queue.extend(sitemap.sitemaps);
            keep::<C>(reading, sitemap.urls);
            if cut {
                reading.done = Some(finish(reading).map(Some));
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
        if !reading.robots.allows(&url) {
            continue;
        }
        if let Some(kind) = C::kind(reading.domain, &url) {
            reading.urls.push((String::from(url), kind, lastmod));
        }
    }
}

/// What `reading` read.
fn finish(reading: &mut Reading<'_>) -> Result<Site, String> {
    let urls = match reading.failed.take().filter(|_| !reading.answered) {
        Some(error) => Err(error),
        None => Ok(mem::take(&mut reading.urls)),
    };
    Ok(Site {
        robots: reading.text.take().map(|mut text| {
            text.truncate(text.floor_char_boundary(ROBOTS));
            text
        }),
        rules: mem::take(&mut reading.robots),
        urls,
    })
}
