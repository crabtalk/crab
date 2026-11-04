//! The runtime of a source service as a kit: the feeds it follows, the items
//! they show and the threads under them, fetched on the service's schedule and
//! stored in its database. Its tables are `lake_items`, `lake_threads` and
//! `lake_records`.
//!
//! A tick's writes and its state commit with the call.

#![no_std]

extern crate alloc;

use alloc::{boxed::Box, collections::BTreeSet, format, string::String, vec::Vec};
use berm_lang::{
    CallError, Result, event,
    http::{self, Response},
    serde::Serialize,
};

pub mod rss;
mod state;
pub mod store;
pub mod time;

pub use state::{Feed, Kind, State, Watch};

/// Bytes a tick stores before it fetches no more threads.
const BUDGET: usize = 8 << 20;

/// When a source fetches. Times are in milliseconds.
pub struct Schedule {
    /// Before a feed is fetched again.
    pub refetch: u64,
    /// Feeds fetched per tick.
    pub feeds: usize,
    /// Threads fetched per tick.
    pub threads: usize,
    /// Before a watched thread is fetched again.
    pub rewarm: u64,
    /// After an item was posted that its thread stays watched.
    pub window: u64,
}

/// One item a feed showed, or one record.
pub struct Listed<T> {
    pub id: String,
    /// When it was posted, in Unix milliseconds; `None` when the feed gives no
    /// date. An undated item is stored and watched as posted when the tick
    /// first stores it. Unread for a record.
    pub posted: Option<u64>,
    pub item: T,
}

/// One GET a source sends, with the other sources' of its round in one batch.
pub struct Request {
    pub url: String,
    pub headers: Vec<(String, String)>,
    /// Sent through the host's proxy, to a host the manifest declares under
    /// `berm.http.proxy`.
    pub proxied: bool,
}

impl Request {
    /// A GET of `url` with no headers, not proxied.
    pub fn get(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            headers: Vec::new(),
            proxied: false,
        }
    }
}

/// The answers to a round's requests, each in its place: a response with a
/// 2xx status, or why there was none.
pub type Answers = Vec<Result<Response, String>>;

/// What a source read from a feed's answer.
pub enum Read<T> {
    /// The items the feed shows.
    Listed(Vec<Listed<T>>),
    /// Requests sent in the next round, then handed their answers.
    More(
        Vec<Request>,
        Box<dyn FnOnce(Answers) -> Result<Read<T>, String>>,
    ),
}

/// A source: what it requests and what it stores.
pub trait Source {
    const SCHEDULE: Schedule;
    type Item;
    type Thread;
    /// An item or a record's values, as stored and answered.
    type Post: Serialize;
    /// A thread as stored and answered.
    type Reply: Serialize;

    /// The request for what `feed` shows now.
    fn request(feed: &Feed) -> Result<Request, String>;

    /// What `feed` shows, read from the answer to its request.
    fn parse(feed: &Feed, response: Response) -> Result<Read<Self::Item>, String>;

    /// `item`, new to `feed`, as stored.
    fn post(feed: &Feed, item: Self::Item) -> Self::Post;

    /// Whether the threads of the items `feed` shows are fetched.
    fn watches(feed: &Feed) -> bool;

    /// The request for the thread under `watch`.
    fn request_thread(watch: &Watch) -> Result<Request, String>;

    /// The thread under `watch` and its comment count, read from the answer to
    /// its request.
    fn parse_thread(watch: &Watch, response: Response) -> Result<(u64, Self::Thread), String>;

    /// `thread`, as stored.
    fn reply(watch: &Watch, thread: Self::Thread) -> Self::Reply;

    /// Follow the feeds the source always follows, at the start of each tick.
    fn follow(_state: &mut State) {}
}

/// A feed fetched in a tick.
#[event]
pub struct Fetched {
    /// The feed's key.
    pub feed: String,
    /// How many items it showed.
    pub shown: u64,
    /// How many of them were new to it and stored.
    pub new: u64,
}

/// A fetch that failed in a tick; it is tried again when next due.
#[event]
pub struct FetchFailed {
    /// The feed's key, or the id of the item whose thread was fetched.
    pub of: String,
    pub error: String,
}

/// One tick of `S`.
pub fn tick<S: Source>() -> Result<()> {
    let schedule = S::SCHEDULE;
    let mut state = State::load()?;
    S::follow(&mut state);
    let now = berm_lang::now();
    let mut stored = 0;
    let State { feeds, watched, .. } = &mut state;
    let mut watching: BTreeSet<String> = watched.iter().map(|watch| watch.id.clone()).collect();

    feeds.sort_by_key(|feed| feed.fetched);
    let mut due: Vec<&mut Feed> = feeds
        .iter_mut()
        .filter(|feed| feed.fetched == 0 || now.saturating_sub(feed.fetched) >= schedule.refetch)
        .take(schedule.feeds)
        .collect();
    for feed in &mut due {
        feed.fetched = now;
    }
    let answers = send(due.iter().map(|feed| S::request(feed)).collect());
    let reads = due
        .iter()
        .zip(answers)
        .map(|(feed, answer)| answer.and_then(|response| S::parse(feed, response)))
        .collect();
    for (feed, read) in due.into_iter().zip(settle(reads)) {
        match read {
            Ok(listed) => stored += posts::<S>(feed, listed, watched, &mut watching, now)?,
            Err(error) => {
                berm_lang::emit!(FetchFailed {
                    of: feed.key.clone(),
                    error,
                })?;
            }
        }
    }

    watched.retain(|watch| now.saturating_sub(watch.posted) < schedule.window);
    watched.sort_by_key(|watch| watch.warmed);
    let due: Vec<&mut Watch> = watched
        .iter_mut()
        .filter(|watch| watch.warmed == 0 || now.saturating_sub(watch.warmed) >= schedule.rewarm)
        .take(schedule.threads)
        .collect();
    let answers = send(due.iter().map(|watch| S::request_thread(watch)).collect());
    for (watch, answer) in due.into_iter().zip(answers) {
        if stored >= BUDGET {
            break;
        }
        watch.warmed = now;
        match answer.and_then(|response| S::parse_thread(watch, response)) {
            Ok((count, _)) if count == watch.comments => {}
            Ok((count, thread)) => {
                watch.comments = count;
                stored += store::thread(&watch.id, now, &S::reply(watch, thread))?;
            }
            Err(error) => {
                berm_lang::emit!(FetchFailed {
                    of: watch.id.clone(),
                    error,
                })?;
            }
        }
    }

    state.save()?;
    Ok(())
}

/// Store the items in `listed` new to `feed`, or the records whose values
/// changed as of `now`, watch them when `S` watches `feed`, and log what it
/// found. `watching` holds the id of each of `watched`. Answers the bytes
/// stored.
fn posts<S: Source>(
    feed: &mut Feed,
    listed: Vec<Listed<S::Item>>,
    watched: &mut Vec<Watch>,
    watching: &mut BTreeSet<String>,
    now: u64,
) -> Result<usize, CallError> {
    let watches = S::watches(feed);
    let mut newest = feed.watermark;
    let shown = listed.len() as u64;
    let mut kept: Vec<(String, u64, S::Post)> = Vec::with_capacity(listed.len());
    for Listed { id, posted, item } in listed {
        newest = newest.max(posted.unwrap_or_default());
        let before = posted.is_some_and(|posted| posted < feed.watermark);
        let posted = posted.unwrap_or(now);
        if watches && watching.insert(id.clone()) {
            watched.push(Watch {
                id: id.clone(),
                feed: feed.key.clone(),
                posted,
                warmed: 0,
                comments: 0,
            });
        }
        if feed.kind == Kind::Chronological && before {
            continue;
        }
        let post = S::post(feed, item);
        kept.push((id, posted, post));
    }
    let (new, stored) = match feed.kind {
        Kind::Chronological | Kind::Unordered => {
            let items: Vec<_> = kept
                .iter()
                .map(|(id, posted, post)| (id.as_str(), *posted, post))
                .collect();
            store::add_items(&feed.key, &items)?
        }
        Kind::Records => {
            let records: Vec<_> = kept
                .iter()
                .map(|(id, _, post)| (id.as_str(), post))
                .collect();
            store::add_records(&feed.key, now, &records)?
        }
    };
    feed.watermark = newest;
    berm_lang::emit!(Fetched {
        feed: feed.key.clone(),
        shown,
        new,
    })?;
    Ok(stored)
}

/// Each of `reads` with its follow-up rounds sent, every read's requests of a
/// round in one batch, until each is the items it lists or why not.
fn settle<T>(reads: Vec<Result<Read<T>, String>>) -> Vec<Result<Vec<Listed<T>>, String>> {
    let mut settled: Vec<Option<Result<Vec<Listed<T>>, String>>> =
        reads.iter().map(|_| None).collect();
    let mut round: Vec<(usize, Result<Read<T>, String>)> = reads.into_iter().enumerate().collect();
    while !round.is_empty() {
        let mut asked = Vec::new();
        let mut pending = Vec::new();
        for (at, read) in round {
            match read {
                Ok(Read::Listed(listed)) => settled[at] = Some(Ok(listed)),
                Err(error) => settled[at] = Some(Err(error)),
                Ok(Read::More(requests, then)) => {
                    pending.push((at, requests.len(), then));
                    asked.extend(requests.into_iter().map(Ok));
                }
            }
        }
        let mut answers = send(asked).into_iter();
        round = pending
            .into_iter()
            .map(|(at, count, then)| (at, then(answers.by_ref().take(count).collect())))
            .collect();
    }
    settled.into_iter().flatten().collect()
}

/// Send `requests`, the plain ones in one `berm.http.fetch_all` and the
/// proxied ones in one `berm.http.proxy_all`, and answer each in its place. A
/// request a source could not build answers why.
fn send(requests: Vec<Result<Request, String>>) -> Answers {
    let mut answers: Vec<Option<Result<Response, String>>> = requests
        .iter()
        .map(|request| request.as_ref().err().map(|error| Err(error.clone())))
        .collect();
    for proxied in [false, true] {
        let batch: Vec<(usize, &Request)> = requests
            .iter()
            .enumerate()
            .filter_map(|(at, request)| Some((at, request.as_ref().ok()?)))
            .filter(|(_, request)| request.proxied == proxied)
            .collect();
        if batch.is_empty() {
            continue;
        }
        let headers: Vec<Vec<(&str, &str)>> = batch
            .iter()
            .map(|(_, request)| {
                request
                    .headers
                    .iter()
                    .map(|(name, value)| (name.as_str(), value.as_str()))
                    .collect()
            })
            .collect();
        let sent: Vec<http::Request<'_>> = batch
            .iter()
            .zip(&headers)
            .map(|((_, request), headers)| http::Request {
                method: "GET",
                url: &request.url,
                headers,
                body: b"",
                timeout: None,
                limit: None,
            })
            .collect();
        let answered = match proxied {
            false => http::fetch_all(&sent),
            true => http::proxy_all(&sent),
        };
        match answered {
            Ok(answered) => {
                for ((at, request), answer) in batch.iter().zip(answered) {
                    let url = &request.url;
                    answers[*at] = Some(match answer {
                        Ok(response) if response.is_success() => Ok(response),
                        Ok(response) => Err(format!("{url}: status {}", response.status)),
                        Err(error) => Err(format!("{url}: {}", error.message())),
                    });
                }
            }
            Err(error) => {
                for (at, request) in &batch {
                    answers[*at] = Some(Err(format!("{}: {}", request.url, error.message())));
                }
            }
        }
    }
    answers.into_iter().flatten().collect()
}
