//! The feed as a lake source, keyed by its URL.

use crate::Post;
use alloc::{format, string::String, vec::Vec};
use berm_lang::http;
use lake::{
    Feed, Listed, Schedule, Source, State, Watch,
    rss::{self, Entry},
};

/// The feed this service follows.
const FEED: &str = "http://127.0.0.1/feed.xml";

pub struct Rss;

impl Source for Rss {
    const SCHEDULE: Schedule = Schedule {
        refetch: 60 * 1000,
        feeds: 1,
        threads: 0,
        rewarm: 0,
        window: 0,
    };
    type Item = Entry;
    type Thread = ();
    type Post = Post;
    type Reply = ();

    fn follow(state: &mut State) {
        state.follow(FEED, true);
    }

    fn fetch(feed: &Feed) -> Result<Vec<Listed<Entry>>, String> {
        let response =
            http::fetch("GET", &feed.key, &[], b"").map_err(|error| format!("{error}"))?;
        if response.status != 200 {
            return Err(format!("{} answered {}", feed.key, response.status));
        }
        let xml = core::str::from_utf8(&response.body)
            .map_err(|_| format!("{} is not UTF-8", feed.key))?;
        Ok(rss::entries(xml)?
            .into_iter()
            .map(|entry| Listed {
                id: entry.id.clone(),
                posted: entry.posted.unwrap_or_default(),
                item: entry,
            })
            .collect())
    }

    fn post(_feed: &Feed, entry: Entry) -> Post {
        Post {
            id: entry.id,
            title: entry.title,
            link: entry.link,
            posted: entry.posted.unwrap_or_default(),
        }
    }

    fn watches(_feed: &Feed) -> bool {
        false
    }

    fn thread(watch: &Watch) -> Result<(u64, ()), String> {
        Err(format!("{} has no thread", watch.id))
    }

    fn reply(_watch: &Watch, _thread: ()) {}
}
