//! The reference source service: one RSS feed, followed through `crab-lake`.
//!
//! `tick` fetches the feed and stores each item new to it, and `posts` reads
//! the stored items back, oldest first.

#![cfg_attr(target_arch = "wasm32", no_std, no_main)]

extern crate alloc;

use alloc::{string::String, vec::Vec};
use berm_lang::Schema;

mod source;

#[berm_lang::program(
    syscalls = { "berm.http.fetch" = ["127.0.0.1"] },
    uses = [lake],
)]
mod tools {
    use crate::{Post, Posts, source::Rss};
    use berm_lang::Result;
    use lake::store;

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
