# Lake

The runtime of a source service: the feeds it follows, the items they show and the threads under them, or the records they list, kept a version per change, fetched on the service's schedule and stored in its database. `rss::entries` reads RSS 2.0, RSS 1.0 and Atom feeds, and `time` reads the dates feeds and pages carry.

## Using it

```toml
[dependencies]
crab-lake = "0.1"
```

The kit declares `berm.get`, `berm.set` and its migrations, which give the program `berm.sql`. The hosts a source fetches from differ per source, so the program declares them under `berm.http.fetch`:

```rust,ignore
#[berm_lang::program(
    uses = [crab_lake],
    syscalls = { "berm.http.fetch" = ["example.com"] },
)]
mod tools {
    use crate::{Post, source::Blog};
    use alloc::vec::Vec;
    use berm_lang::Result;
    use crab_lake::store;

    /// Fetches the feeds due and stores each item new to one.
    #[cron]
    pub fn tick() -> Result<()> {
        crab_lake::tick::<Blog>()
    }

    /// Lists the posts stored after `after`, oldest first.
    #[query]
    #[public]
    pub fn posts(after: u64) -> Result<Vec<Post>> {
        let stored = store::items::<Post>(None, after, 50)?;
        Ok(stored.into_iter().map(|post| post.item).collect())
    }
}
```

Set the tick's expression in `[crons]` of the service's `berm.toml`:

```toml
[crons]
tick = "*/5 * * * *"
```

## A source

Implement `Source` to fetch items and threads and shape what is stored, and set a `Schedule` with fetch limits and intervals in milliseconds:

```rust,ignore
use crab_lake::{Feed, Kind, Listed, Schedule, Source, State, Watch, rss::{self, Entry}};

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

    fn fetch(feed: &Feed) -> Result<Vec<Listed<Entry>>, String> {
        let response = berm_lang::http::fetch("GET", &feed.key, &[], b"").map_err(|e| format!("{e}"))?;
        let xml = core::str::from_utf8(&response.body).map_err(|_| String::from("not UTF-8"))?;
        Ok(rss::entries(xml)?
            .into_iter()
            .map(|entry| Listed { id: entry.id.clone(), posted: entry.posted.unwrap_or_default(), item: entry })
            .collect())
    }

    fn post(_feed: &Feed, entry: Entry) -> Post { /* ... */ }
    fn watches(_feed: &Feed) -> bool { false }
    fn thread(watch: &Watch) -> Result<(u64, ()), String> { Err(format!("{} has no thread", watch.id)) }
    fn reply(_watch: &Watch, _thread: ()) {}
}
```

`Source::follow` follows the feeds a source always follows, at the start of each tick. `State::follow` and `State::unfollow`, between `State::load` and `State::save`, change the feeds followed from the service's own tools. A feed's `Kind` says what of it is stored: `Chronological`, the items posted after the newest it has shown; `Unordered`, the items it has not stored before, which `store::has` answers; `Records`, each entry as a record, a version stored when its id is new to the feed or its values, the `Post`, changed. `store::record(id, feed, now, values)` stores a version the same way outside a fetch, in a feed the program names, and answers the bytes written.

## A tick

Each tick fetches the feeds due, least recently fetched first, stores each item new to one, and watches its thread when `Source::watches` says so. It then fetches the threads due and stores each whose comment count changed in place of the last, until the tick has stored 8 MiB. It logs a `Fetched` for each feed and a `FetchFailed` for each fetch that failed, which is retried when next due. State is stored under the service's `lake` key, and all of it commits with the call.

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
