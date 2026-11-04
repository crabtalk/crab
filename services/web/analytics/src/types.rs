use alloc::{string::String, vec::Vec};
use berm_lang::Schema;

/// What `record` answers.
#[derive(Schema)]
pub struct Recorded {
    /// `false` for a crawler report's request from no AI crawler.
    pub kept: bool,
}

/// One day's views.
#[derive(Schema)]
pub struct Day {
    /// Midnight UTC starting it, in Unix milliseconds.
    pub day: u64,
    /// Pages people opened.
    pub views: u64,
    /// People who opened a page, each counted once.
    pub visitors: u64,
    /// Of `views`, those referred by an AI assistant.
    pub referred: u64,
    /// Requests from AI crawlers.
    pub crawls: u64,
}

/// What `days` answers.
#[derive(Schema)]
pub struct Days {
    pub days: Vec<Day>,
}

/// One page and its views.
#[derive(Schema)]
pub struct Page {
    pub page: String,
    pub views: u64,
    /// People who opened it, each counted once a day.
    pub visitors: u64,
}

/// What `pages` answers.
#[derive(Schema)]
pub struct Pages {
    pub pages: Vec<Page>,
}

/// One referring host and the views it sent.
#[derive(Schema)]
pub struct Source {
    pub host: String,
    /// Whether the host is an AI assistant's.
    pub assistant: bool,
    pub views: u64,
    /// People it sent, each counted once a day.
    pub visitors: u64,
}

/// What `sources` answers.
#[derive(Schema)]
pub struct Sources {
    pub sources: Vec<Source>,
}

/// One AI crawler and its requests.
#[derive(Schema)]
pub struct Crawl {
    /// Its user agent token, like `GPTBot`.
    pub crawler: String,
    pub requests: u64,
}

/// What `crawlers` answers.
#[derive(Schema)]
pub struct Crawlers {
    pub crawlers: Vec<Crawl>,
}

/// One recorded view.
#[derive(Schema)]
pub struct Event {
    /// Its position among every view recorded; what `next` is made of.
    pub seq: u64,
    /// When it was recorded, in Unix milliseconds.
    pub at: u64,
    pub page: String,
    /// The host that referred it.
    pub source: Option<String>,
    /// The AI crawler that made it.
    pub crawler: Option<String>,
}

/// What `events` answers.
#[derive(Schema)]
pub struct Events {
    pub events: Vec<Event>,
    /// What to pass as `after` for the views recorded since.
    pub next: u64,
}

/// Why a call is refused.
#[berm_lang::error_code]
pub enum AnalyticsError {
    #[msg("no such site")]
    NoSite,
    #[msg("a domain is a host name, like example.com")]
    BadDomain,
    #[msg("a name is 1 to 120 characters")]
    BadName,
    #[msg("you already added that domain")]
    SiteTaken,
    #[msg("a page is a path starting with /, up to 1024 bytes")]
    BadPage,
}
