# Crawl

An incremental crawler for berm programs: the domains a program watches, each site's robots.txt and sitemaps, and the pages of the kinds the program names, fetched on its schedule into its database and recrawled when they change.

## Using it

```toml
[dependencies]
crabkit = { version = "0.1", features = ["crawl"] }
```

The kit is also the crate `crabkit-crawl` on its own.

The kit's `berm.toml` declares `"berm.http.fetch" = ["*"]` and its migrations, which give the program `berm.sql`; they reach the program's manifest when the program links the kit. The local host refuses a host matched only by a pattern when it resolves to an address that is not public.

```rust,ignore
use berm_lang::{Result, tool};

/// Reads the sites due and fetches the pages due.
#[tool(cron)]
pub fn crawl() -> Result<()> {
    crabkit::crawl::tick::<Pages>()
}
```

## A crawler

Implement `Crawler` with a `Schedule`, its `User-Agent`, and the kind of each URL a sitemap lists that it fetches:

```rust,ignore
use crabkit::crawl::{Crawler, Schedule};
use url::Url;

pub struct Pages;

impl Crawler for Pages {
    const SCHEDULE: Schedule = Schedule {
        recrawl: 7 * 24 * 60 * 60 * 1000,
        refetch: 30 * 24 * 60 * 60 * 1000,
        sites: 20,
        sitemaps: 4 << 20,
        pages: 50,
        html: 2 << 20,
        page: 256 << 10,
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
- `list::<C>(domain, urls, bytes)` keeps each URL on a watched site that is of a kind `C` names, fetched as the site's sitemap URLs are once the site is read, each read for `bytes` of HTML when given. A sitemap read leaves it, and a robots.txt that disallows it drops it. `Crawler::read` may list the links of the page it is handed, in the same call.
- Each request fails when nothing answered within `Schedule::timeout` milliseconds. A site that answered a page with nothing is fetched no more that tick.
- A site is read every `recrawl`: its robots.txt, then the sitemaps it names, else `/sitemap.xml`, at most 12 sitemap files, `.gz` ones skipped. The URLs of a kind that robots.txt allows replace the site's last. robots.txt is read as RFC 9309 says, the group for the token before the `/` in `AGENT`, else `*`, its patterns matched against the URL's path and query percent-encoded, octets outside ASCII encoded and escapes in uppercase hex; a 4xx allows everything, and a 5xx or a failed request fails the read. A read whose sitemaps timed out or answered 5xx, none answering that names no sitemaps or lists URLs, keeps the site's last URLs and the failure as its error, and logs it in `Read`'s `sitemaps`; a sitemap answering anything else but 200 is absent, and a site whose sitemaps are all absent has no URLs. A failed read is tried again on a later tick until `failures` in a row, and then `recrawl` after the last. A site keeps its last error and when it failed, a page that answered nothing its last error.
- A page is fetched when new, when its URL's `lastmod` changed, or, with no `lastmod`, `refetch` after it last was, with `If-None-Match` and `If-Modified-Since` from what it last answered. A redirect is followed within the site, five at most. Sites watched first are read and fetched before the rest.
- A page keeps its title (`<title>`, else `og:title`), its description (`description`, else `og:description`) and up to 64 KiB of its text. A page that holds no text, such as one rendered by JavaScript, is stored without text.
- `Crawler::read` is handed each page that answered `200` with HTML, its domain, URL, kind and HTML, in the call that stored it, and answers the bytes it stored itself; by default it stores nothing.
- A site sees one request at a time; a tick's requests go out in batches over `berm.http.fetch_all`.
- A tick stops at 8 MiB stored, or at `html` bytes of HTML read, and leaves the rest due; all of it commits with the call. The first site due is read for `sitemaps` bytes of sitemaps; the rest share as many, and one cut at its share is left due. A page is read for its first `page` bytes of HTML, or the bytes it was listed with; one listed with more than the tick's HTML has left is left due, unless it is the tick's first, which is read past `html`. A robots.txt is read for its first 500 KiB. Each request takes at most the bytes left for it, and the host cuts the rest of its answer.
- A tick logs `Read` for each site read and `ReadFailed` and `FetchFailed` for each failure.
- `store::site`, `store::urls` and `store::page` read back a site, its URLs by kind, and a page; `store::failed` the sites whose last read failed.

## License

Apache-2.0
