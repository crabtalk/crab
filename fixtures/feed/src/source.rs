//! The feed as a lake source, keyed by its URL, and any feed of records `list`
//! follows, read the same way.

use crate::Post;
use alloc::{format, string::String};
use berm_lang::http::Response;
use core::str;
use lake::{
    Feed, Kind, Listed, Read, Request, Schedule, Source, State, Watch,
    rss::{self, Entry},
};

/// The feed this service follows.
const FEED: &str = "http://127.0.0.1/feed.xml";

pub struct Rss;

impl Source for Rss {
    const SCHEDULE: Schedule = Schedule {
        refetch: 60 * 1000,
        feeds: 2,
        threads: 0,
        rewarm: 0,
        window: 0,
    };
    type Item = Entry;
    type Thread = ();
    type Post = Post;
    type Reply = ();

    fn follow(state: &mut State) {
        state.follow(FEED, Kind::Chronological);
    }

    fn request(feed: &Feed) -> Result<Request, String> {
        Ok(Request::get(&feed.key))
    }

    fn parse(feed: &Feed, response: Response) -> Result<Read<Entry>, String> {
        let xml =
            str::from_utf8(&response.body).map_err(|_| format!("{} is not UTF-8", feed.key))?;
        Ok(Read::Listed(
            rss::entries(xml)?
                .into_iter()
                .map(|entry| Listed {
                    id: entry.id.clone(),
                    posted: entry.posted,
                    item: entry,
                })
                .collect(),
        ))
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

    fn request_thread(watch: &Watch) -> Result<Request, String> {
        Err(format!("{} has no thread", watch.id))
    }

    fn parse_thread(watch: &Watch, _response: Response) -> Result<(u64, ()), String> {
        Err(format!("{} has no thread", watch.id))
    }

    fn reply(_watch: &Watch, _thread: ()) {}
}
