//! The runtime of a source service as a kit: the feeds it follows, the items
//! they show and the threads under them, fetched on the service's schedule and
//! stored in its database. Its tables are `lake_items` and `lake_threads`.
//!
//! A tick fetches the feeds due, least recently fetched first, and stores each
//! item new to one. It then fetches the threads due, least recently fetched
//! first, and stores each whose comment count changed in place of the last. It
//! saves its state; all of it commits with the call. It logs what each fetch
//! found and each fetch that failed, which is tried again when next due.

#![no_std]

extern crate alloc;

use alloc::{string::String, vec::Vec};
use berm_lang::{CallError, Result, event, serde::Serialize};

pub mod rss;
mod state;
pub mod store;
pub mod time;

pub use state::{Feed, State, Watch};

berm_lang::kit!(
    syscalls = { "berm.get", "berm.set" },
    database = "migrations",
);

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

/// One item a feed showed.
pub struct Listed<T> {
    pub id: String,
    /// When it was posted, in Unix milliseconds.
    pub posted: u64,
    pub item: T,
}

/// A source: how it fetches and what it stores.
pub trait Source {
    const SCHEDULE: Schedule;
    type Item;
    type Thread;
    /// An item as stored and answered.
    type Post: Serialize;
    /// A thread as stored and answered.
    type Reply: Serialize;

    /// The items `feed` shows now.
    fn fetch(feed: &Feed) -> Result<Vec<Listed<Self::Item>>, String>;

    /// `item`, new to `feed`, as stored.
    fn post(feed: &Feed, item: Self::Item) -> Self::Post;

    /// Whether the threads of the items `feed` shows are fetched.
    fn watches(feed: &Feed) -> bool;

    /// The thread under `watch` and its comment count.
    fn thread(watch: &Watch) -> Result<(u64, Self::Thread), String>;

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

    feeds.sort_by_key(|feed| feed.fetched);
    let due = feeds
        .iter_mut()
        .filter(|feed| feed.fetched == 0 || now.saturating_sub(feed.fetched) >= schedule.refetch)
        .take(schedule.feeds);
    for feed in due {
        feed.fetched = now;
        match S::fetch(feed) {
            Ok(listed) => stored += posts::<S>(feed, listed, watched)?,
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
    let due = watched
        .iter_mut()
        .filter(|watch| watch.warmed == 0 || now.saturating_sub(watch.warmed) >= schedule.rewarm)
        .take(schedule.threads);
    for watch in due {
        if stored >= BUDGET {
            break;
        }
        watch.warmed = now;
        match S::thread(watch) {
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

/// Store the items in `listed` new to `feed`, watch them when `S` watches
/// `feed`, record what it showed, and log what it found. Answers the bytes
/// stored.
fn posts<S: Source>(
    feed: &mut Feed,
    listed: Vec<Listed<S::Item>>,
    watched: &mut Vec<Watch>,
) -> Result<usize, CallError> {
    let watches = S::watches(feed);
    let mut newest = feed.watermark;
    let (mut stored, mut new, count) = (0, 0, listed.len() as u64);
    for Listed { id, posted, item } in listed {
        newest = newest.max(posted);
        if watches && !watched.iter().any(|watch| watch.id == id) {
            watched.push(Watch {
                id: id.clone(),
                feed: feed.key.clone(),
                posted,
                warmed: 0,
                comments: 0,
            });
        }
        let fresh = match feed.chronological {
            true => posted > feed.watermark,
            false => !store::has(&feed.key, &id)?,
        };
        if fresh {
            let written = store::item(&id, &feed.key, posted, &S::post(feed, item))?;
            stored += written;
            new += u64::from(written > 0);
        }
    }
    feed.watermark = newest;
    berm_lang::emit!(Fetched {
        feed: feed.key.clone(),
        shown: count,
        new,
    })?;
    Ok(stored)
}
