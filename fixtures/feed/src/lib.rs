//! The reference source service: one RSS feed, followed through `crabkit-lake`.
//!
//! `tick` fetches the feed and stores each item new to it, and `posts` reads
//! the stored items back, oldest first. `list` follows a feed of records, each
//! item a record keyed by its guid, and `record` and `versions` read them
//! back.

#![cfg_attr(target_arch = "wasm32", no_std, no_main)]

extern crate alloc;

use alloc::{string::String, vec::Vec};
use berm_lang::Schema;
use lake::store::Version;

mod source;

#[berm_lang::program(
    syscalls = { "berm.http.fetch" = ["127.0.0.1"] },
    uses = [lake],
)]
mod tools {
    use crate::{Post, Posts, Records, source::Rss};
    use alloc::string::String;
    use berm_lang::Result;
    use lake::{Kind, State, store};

    /// Fetches the feed and stores each item new to it.
    #[cron]
    pub fn tick() -> Result<()> {
        lake::tick::<Rss>()
    }

    /// Lists the items stored after `after`, oldest first.
    #[query]
    #[public]
    pub fn posts(
        /// The `next` of the last page; 0 or absent for the first.
        after: Option<u64>,
        /// At most this many; 50 by default.
        limit: Option<u64>,
    ) -> Result<Posts> {
        let after = after.unwrap_or(0);
        let stored = store::items::<Post>(None, after, limit.unwrap_or(50))?;
        Ok(Posts {
            next: stored.last().map_or(after, |post| post.seq),
            posts: stored.into_iter().map(|post| post.item).collect(),
        })
    }

    /// Follows the feed of records at `url`. Answers whether it was new.
    #[public]
    pub fn list(url: String) -> Result<bool> {
        let mut state = State::load()?;
        let added = state.follow(&url, Kind::Records);
        state.save()?;
        Ok(added)
    }

    /// The latest version of record `id` in each feed that showed it.
    #[query]
    #[public]
    pub fn record(id: String) -> Result<Records> {
        Ok(Records::from(store::record_of::<Post>(&id)?))
    }

    /// Every version of record `id`, oldest first.
    #[query]
    #[public]
    pub fn versions(id: String) -> Result<Records> {
        Ok(Records::from(store::versions_of::<Post>(&id, None)?))
    }
}

/// What `record` and `versions` answer.
#[derive(Schema)]
pub struct Records {
    pub records: Vec<Recorded>,
}

impl From<Vec<Version<Post>>> for Records {
    fn from(versions: Vec<Version<Post>>) -> Self {
        Records {
            records: versions
                .into_iter()
                .map(|version| Recorded {
                    feed: version.feed,
                    since: version.since,
                    post: version.values,
                })
                .collect(),
        }
    }
}

/// A version of a record.
#[derive(Schema)]
pub struct Recorded {
    pub feed: String,
    /// When it was stored, in Unix milliseconds.
    pub since: u64,
    pub post: Post,
}

/// What `posts` answers.
#[derive(Schema)]
pub struct Posts {
    pub posts: Vec<Post>,
    /// What to pass as `after` for the items stored since.
    pub next: u64,
}

/// One item of the feed, as stored.
#[derive(Schema)]
pub struct Post {
    pub id: String,
    pub title: String,
    pub link: String,
    /// When it was posted, in Unix milliseconds.
    pub posted: u64,
}
