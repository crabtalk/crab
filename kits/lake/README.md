# Lake

The runtime of a source service: the feeds it follows, the items they show and the threads under them, or the records they list, kept a version per change, fetched on the service's schedule and stored in its database. `rss::entries` reads RSS 2.0, RSS 1.0 and Atom feeds, and `time` reads the dates feeds and pages carry.

## Using it

```toml
[dependencies]
crabkit = { version = "0.1", features = ["lake"] }
```

The kit is also the crate `crabkit-lake` on its own.

The kit's `crab.toml` declares `berm.get`, `berm.set` and its migrations, which give the program `berm.sql`; they reach the program's manifest when the program links the kit. The hosts a source fetches from differ per source, so the program declares them under `berm.http.fetch` in its own `crab.toml`, beside the tick's expression in `[crons]`:

```toml
[syscalls]
"berm.http.fetch" = ["example.com"]

[crons]
tick = "*/5 * * * *"
```

```rust,ignore
use crate::{Post, source::Blog};
use alloc::vec::Vec;
use berm_lang::{Result, tool};
use crabkit::lake::store;

/// Fetches the feeds due and stores each item new to one.
#[tool(cron)]
pub fn tick() -> Result<()> {
    crabkit::lake::tick::<Blog>()
}

/// Lists the posts stored after `after`, oldest first.
#[tool(query, public)]
pub fn posts(after: u64) -> Result<Vec<Post>> {
    let stored = store::items::<Post>(None, after, 50)?;
    Ok(stored.into_iter().map(|post| post.item).collect())
}
```

## A source

Implement `Source` to build the request for a feed or a thread and read its answer, and shape what is stored, and set a `Schedule` with fetch limits and intervals in milliseconds:

```rust,ignore
use core::str;
use berm_lang::http::Response;
use crabkit::lake::{Feed, Kind, Listed, Read, Request, Schedule, Source, State, Watch, rss::{self, Entry}};

pub struct Blog;

impl Source for Blog {
    const SCHEDULE: Schedule = Schedule {
        refetch: 5 * 60 * 1000,
        feeds: 10,
        threads: 0,
        rewarm: 0,
        window: 0,
    };
    type Item = Entry;
    type Thread = ();
    type Post = Post;
    type Reply = ();

    fn follow(state: &mut State) {
        state.follow("https://example.com/feed.xml", Kind::Chronological);
    }

    fn request(feed: &Feed) -> Result<Request, String> {
        Ok(Request::get(&feed.key))
    }

    fn parse(_feed: &Feed, response: Response) -> Result<Read<Entry>, String> {
        let xml = str::from_utf8(&response.body).map_err(|_| String::from("not UTF-8"))?;
        Ok(Read::Listed(rss::entries(xml)?
            .into_iter()
            .map(|entry| Listed { id: entry.id.clone(), posted: entry.posted, item: entry })
            .collect()))
    }

    fn post(_feed: &Feed, entry: Entry) -> Post { /* ... */ }
    fn watches(_feed: &Feed) -> bool { false }
    fn request_thread(watch: &Watch) -> Result<Request, String> { Err(format!("{} has no thread", watch.id)) }
    fn parse_thread(watch: &Watch, _response: Response) -> Result<(u64, ()), String> { Err(format!("{} has no thread", watch.id)) }
    fn reply(_watch: &Watch, _thread: ()) {}
}
```

`Source::follow` follows the feeds a source always follows, at the start of each tick. `State::follow` and `State::unfollow`, between `State::load` and `State::save`, change the feeds followed from the service's own tools. A feed's `Kind` says what of it is stored: `Chronological`, the items undated or not posted before the newest it has shown, each once; `Unordered`, the items it has not stored before; `Records`, each entry as a record, a version stored when its id is new to the feed or its values, the `Post`, changed. `store::record(id, feed, now, values)` stores a version the same way outside a fetch, in a feed the program names, and answers the bytes written.

## A tick

A tick fetches the feeds due and then the threads due, least recently fetched first, each phase's requests sent in one batch: plain ones over `berm.http.fetch_all`, proxied ones over `berm.http.proxy_all`. An answer with a status outside 2xx is a failed fetch. A feed whose `parse` answers `Read::More` has those requests sent in the next round, with every other feed's of that round, and its closure handed their answers, until it answers `Read::Listed`. It stores no more threads once it has stored 8 MiB. A thread is stored in place of the last only when its comment count changed. It logs a `Fetched` for each feed and a `FetchFailed` for each fetch that failed, which is retried when next due. State is stored under the service's `lake` key, and all of it commits with the call.

## Reading

- `store::items::<T>(feed, after, limit)`: up to `limit` items stored after `after`, oldest first, of `feed` when given. Each is a `Stored { seq, item }`; pass the last `seq` as `after` for the next page.
- `store::thread_of::<T>(id)`: the last thread stored under item `id`.
- `store::record_of::<T>(id)`: the latest version of record `id` in each feed that showed it. Each is a `Version { seq, id, feed, since, values }`.
- `store::versions_of::<T>(id, feed)`: every version of record `id`, of `feed` when given, oldest first.
- `store::versions::<T>(feed, after, limit)`: up to `limit` versions stored after `after`, oldest first, of `feed` when given.

## Tables

In the program's database: `lake_items` (`seq`, `id`, `feed`, `posted`, `data`), one row per item per feed, indexed by `lake_items_feed` on (`feed`, `seq`); `lake_threads` (`id`, `data`, `fetched`), the last thread fetched under each watched item; and `lake_records` (`seq`, `id`, `feed`, `since`, `data`), one row per version of a record, indexed by `lake_records_id` on (`id`, `feed`, `seq`). `data` is the `Post` or `Reply` as JSON.

## License

Apache-2.0
