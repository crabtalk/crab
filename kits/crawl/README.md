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
        pages: 50,
        failures: 3,
    };
    const AGENT: &'static str = "example/0.1";

    fn kind(_domain: &str, url: &Url) -> Option<&'static str> {
        (url.path() == "/pricing").then_some("pricing")
    }
}
```

- `watch(domains)` watches each domain; one already watched is left as it is.
- A tick reads the sites due, never read or read `recrawl` ago, least recently read first: robots.txt, then the sitemaps it names, else `/sitemap.xml`, at most 12 sitemap files, `.gz` ones skipped. The URLs of a kind that robots.txt allows replace the site's last. robots.txt is read as RFC 9309 says, the group for the token before the `/` in `AGENT`, else `*`; a 4xx allows everything, and a 5xx or a failed request fails the read, which is tried again on a later tick until `failures` in a row.
- It then fetches the pages due, least recently fetched first: new, their URL's `lastmod` changed, or with no `lastmod` and fetched `refetch` ago, with `If-None-Match` and `If-Modified-Since` from what each last answered. A redirect is followed within the site, five at most.
- A page keeps its title (`<title>`, else `og:title`), its description (`description`, else `og:description`) and up to 64 KiB of its text. A page that holds no text, such as one rendered by JavaScript, is stored without text.
- A tick stops at 8 MiB stored and leaves the rest due. It logs `Read` for each site read and `ReadFailed` and `FetchFailed` for each failure.
- `store::site`, `store::urls` and `store::page` read back a site, its URLs by kind, and a page.

## License

Apache-2.0
