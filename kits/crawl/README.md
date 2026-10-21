# Crawl

An incremental crawler for berm programs: the domains a program watches, each site's robots.txt and sitemaps, and the pages of the kinds the program names, fetched on its schedule into its database and recrawled when they change.

## Using it

```toml
[dependencies]
crab-crawl = "0.1"
```

The kit declares `"berm.http.fetch" = ["*"]` and its migrations, which give the program `berm.sql`. bermd refuses a host matched only by a pattern when it resolves to an address that is not public.

```rust,ignore
#[berm_lang::program(uses = [crab_crawl])]
mod tools {
    use crate::Pages;
    use berm_lang::Result;

    /// Reads the sites due and fetches the pages due.
    #[cron]
    pub fn crawl() -> Result<()> {
        crab_crawl::tick::<Pages>()
    }
}
```

## A crawler

Implement `Crawler` with a `Schedule`, its `User-Agent`, and the kind of each URL a sitemap lists that it fetches:

```rust,ignore
use crab_crawl::{Crawler, Schedule};
use url::Url;

pub struct Pages;

impl Crawler for Pages {
    const SCHEDULE: Schedule = Schedule {
        recrawl: 7 * 24 * 60 * 60 * 1000,
        refetch: 30 * 24 * 60 * 60 * 1000,
        sites: 20,
        listed: 20_000,
        pages: 50,
        failures: 3,
        timeout: 10_000,
    };
    const AGENT: &'static str = "example/0.1";

    fn kind(_domain: &str, url: &Url) -> Option<&'static str> {
        (url.path() == "/pricing").then_some("pricing")
    }
}
```

- `watch(domains)` watches each domain; one already watched is left as it is. `watch_first(domains)` watches each to be read before the other sites due.
- Each request fails when nothing answered within `Schedule::timeout` milliseconds. A site that answered a page with nothing is fetched no more that tick.
- A tick reads the sites due, never read or read `recrawl` ago, those watched first before the rest, least recently read first: robots.txt, then the sitemaps it names, else `/sitemap.xml`, at most 12 sitemap files, `.gz` ones skipped. The URLs of a kind that robots.txt allows replace the site's last. robots.txt is read as RFC 9309 says, the group for the token before the `/` in `AGENT`, else `*`; a 4xx allows everything, and a 5xx or a failed request fails the read, which is tried again on a later tick until `failures` in a row, and then `recrawl` after the last. A site keeps its last error and when it failed, a page that answered nothing its last error.
- It then fetches the pages due, least recently fetched first: new, their URL's `lastmod` changed, or with no `lastmod` and fetched `refetch` ago, with `If-None-Match` and `If-Modified-Since` from what each last answered. A redirect is followed within the site, five at most.
- A page keeps its title (`<title>`, else `og:title`), its description (`description`, else `og:description`) and up to 64 KiB of its text. A page that holds no text, such as one rendered by JavaScript, is stored without text.
- `Crawler::read` is handed each page that answered `200` with HTML, its domain, URL, kind and HTML, in the call that stored it, and answers the bytes it stored itself; by default it stores nothing.
- A tick reads its sites at once and fetches its pages a page a site a round, each round's requests sent as one batch over `berm.http.fetch_all`, so a site sees one request at a time. Each page of a round is read for its share of the HTML left.
- A tick stops at 8 MiB stored, or at 2 MiB of HTML read, and leaves the rest due. The first site due is read for `listed` sitemap URLs of its own; the rest share `listed`, and one that would list more than are left is left due. It logs `Read` for each site read and `ReadFailed` and `FetchFailed` for each failure.
- `store::site`, `store::urls` and `store::page` read back a site, its URLs by kind, and a page; `store::failed` the sites whose last read failed.

## License

Apache-2.0
