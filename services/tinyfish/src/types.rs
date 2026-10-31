use alloc::{
    collections::BTreeMap,
    format,
    string::{String, ToString},
    vec::Vec,
};
use berm_lang::{
    Error, Result, Schema,
    serde::{Deserialize, Serialize},
    serde_json::{self, Value},
};
use url::Url;

#[derive(Debug, Clone, Default, Schema)]
pub struct SearchOptions {
    /// Zero-based result page, from 0 to 10.
    pub page: Option<u8>,
    pub purpose: Option<String>,
    /// Country code, such as US or GB.
    pub location: Option<String>,
    /// Language code, such as en or fr.
    pub language: Option<String>,
    pub include_domains: Option<Vec<String>>,
    pub exclude_domains: Option<Vec<String>>,
    /// YYYY-MM-DD; web and news searches only.
    pub after_date: Option<String>,
    /// YYYY-MM-DD; web and news searches only.
    pub before_date: Option<String>,
    /// Freshness in minutes; cannot be combined with date filters.
    pub recency_minutes: Option<u32>,
    /// Publication-year bounds for research-paper searches.
    pub pub_year_min: Option<u16>,
    pub pub_year_max: Option<u16>,
    pub include_thumbnail: Option<bool>,
    /// Fetch configuration attached to each search result; an empty object enables defaults.
    pub fetch: Option<BTreeMap<String, Value>>,
}

#[derive(Debug, Clone, Schema)]
pub struct SearchPage {
    pub query: String,
    pub results: Vec<SearchResult>,
    /// Number of results on this page.
    pub total_results: u32,
    pub page: u8,
    pub request_id: Option<String>,
}

#[derive(Debug, Clone, Schema)]
pub struct SearchResult {
    pub position: u32,
    pub site_name: String,
    pub title: String,
    pub snippet: String,
    pub url: String,
    pub date: Option<String>,
    pub publisher: Option<String>,
    pub authors: Option<Vec<String>>,
    pub venue: Option<String>,
    pub year: Option<u16>,
    pub cited_by_count: Option<u64>,
    pub pdf_url: Option<String>,
    pub thumbnail_url: Option<String>,
    pub fetch: Option<Value>,
    pub fetch_error: Option<Value>,
}

#[derive(Debug, Clone, Default, Schema)]
pub enum Format {
    #[default]
    Markdown,
    Html,
    Json,
}

#[derive(Debug, Clone, Default, Schema)]
pub struct FetchOptions {
    pub purpose: Option<String>,
    pub format: Option<Format>,
    pub links: Option<bool>,
    pub image_links: Option<bool>,
    pub page_metadata: Option<bool>,
    /// Cache tolerance in seconds. Omit to accept cached content; 0 requests live content.
    pub ttl: Option<u32>,
    /// From 1 to 110000 milliseconds. Omit to use the provider default.
    pub per_url_timeout_ms: Option<u32>,
    /// Conditional fetching accepts exactly one URL.
    pub if_none_match: Option<String>,
    pub if_modified_since: Option<String>,
    pub include_etag_and_last_modified: Option<bool>,
    /// Up to 20 CSS selectors, each at most 1000 characters.
    pub include_selectors: Option<Vec<String>>,
    pub exclude_selectors: Option<Vec<String>>,
}

#[derive(Debug, Clone, Schema)]
pub struct FetchResult {
    pub results: Vec<Page>,
    /// Partial URL failures remain alongside successful pages.
    pub errors: Vec<PageError>,
    pub request_id: Option<String>,
}

#[derive(Debug, Clone, Schema)]
pub struct Page {
    pub url: String,
    pub final_url: Option<String>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub language: Option<String>,
    pub author: Option<String>,
    pub published_date: Option<String>,
    pub format: Format,
    /// A string for Markdown/HTML, a document tree for JSON, or null for unchanged content.
    pub text: Value,
    pub links: Option<Vec<String>>,
    pub image_links: Option<Vec<String>>,
    pub page_metadata: Option<BTreeMap<String, Value>>,
    pub latency_ms: Option<f64>,
    pub not_modified: Option<bool>,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
    pub unmatched_selectors: Option<Vec<String>>,
}

#[derive(Debug, Clone, Schema)]
pub struct PageError {
    pub url: String,
    pub error: String,
    pub status: Option<u16>,
    pub unmatched_selectors: Option<Vec<String>>,
    pub candidate_selectors: Option<Vec<String>>,
}

#[derive(Debug, Clone, Default, Schema)]
pub enum BrowserProfile {
    #[default]
    Lite,
    Stealth,
}

#[derive(Debug, Clone, Default, Schema)]
pub enum AgentMode {
    #[default]
    Default,
    Strict,
}

#[derive(Debug, Clone, Default, Schema)]
pub struct AgentConfig {
    pub mode: Option<AgentMode>,
    /// fish, standard, or green-pointer.
    pub cursor_style: Option<String>,
    /// From 1 to 500; custom limits may require TinyFish beta access.
    pub max_steps: Option<u16>,
    pub max_duration_seconds: Option<u32>,
}

#[derive(Debug, Clone, Default, Schema)]
pub struct CaptureConfig {
    pub elements: Option<bool>,
    pub snapshots: Option<bool>,
    pub screenshots: Option<bool>,
    pub recording: Option<bool>,
    pub html: Option<bool>,
}

#[derive(Debug, Clone, Schema)]
pub struct ProxyConfig {
    pub enabled: bool,
    /// Proxy exit country, such as US or GB.
    pub country_code: Option<String>,
}

#[derive(Debug, Clone, Default, Schema)]
pub struct AgentOptions {
    pub browser_profile: Option<BrowserProfile>,
    pub agent_config: Option<AgentConfig>,
    pub capture_config: Option<CaptureConfig>,
    pub proxy_config: Option<ProxyConfig>,
    /// Provider-supported JSON Schema for the terminal result.
    pub output_schema: Option<BTreeMap<String, Value>>,
}

#[derive(Debug, Clone, Schema)]
pub struct StartedRun {
    pub run_id: String,
}

#[derive(Debug, Clone, Schema)]
pub struct Run {
    pub run_id: String,
    /// PENDING, RUNNING, COMPLETED, FAILED or CANCELLED.
    pub status: String,
    pub goal: Option<String>,
    pub created_at: Option<String>,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub num_of_steps: Option<u32>,
    pub result: Option<Value>,
    pub error: Option<Value>,
    pub steps: Option<Vec<Value>>,
    pub output_schema: Option<Value>,
    pub streaming_url: Option<String>,
    pub video_url: Option<String>,
    pub browser_config: Option<Value>,
    pub profile_attached: Option<bool>,
    pub profile_id: Option<String>,
    pub profile_hint: Option<Value>,
}

#[derive(Debug, Clone, Schema)]
pub struct CancelledRun {
    pub run_id: String,
    pub status: String,
    pub cancelled_at: Option<String>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Schema)]
pub struct Screenshot {
    pub content_type: String,
    pub base64: String,
}

impl SearchOptions {
    pub(crate) fn url(&self, origin: &str, query: &str, domain: &str) -> Result<Url> {
        if query.trim().is_empty() {
            return Err("Provide a search query".into());
        }
        if self.page.is_some_and(|page| page > 10) {
            return Err("Search page must be from 0 to 10".into());
        }
        let mut url = Url::parse(origin).expect("constant URL");
        {
            let mut params = url.query_pairs_mut();
            params
                .append_pair("query", query)
                .append_pair("domain_type", domain);
            for (name, value) in [
                ("after_date", self.after_date.as_deref()),
                ("before_date", self.before_date.as_deref()),
                ("language", self.language.as_deref()),
                ("location", self.location.as_deref()),
                ("purpose", self.purpose.as_deref()),
            ] {
                if let Some(value) = value {
                    params.append_pair(name, value);
                }
            }
            for (name, value) in [
                (
                    "exclude_domains",
                    self.exclude_domains.as_ref().map(|v| v.join(",")),
                ),
                (
                    "fetch",
                    self.fetch.as_ref().map(serde_json::to_string).transpose()?,
                ),
                (
                    "include_domains",
                    self.include_domains.as_ref().map(|v| v.join(",")),
                ),
                (
                    "include_thumbnail",
                    self.include_thumbnail.map(|v| v.to_string()),
                ),
                ("page", self.page.map(|v| v.to_string())),
                ("pub_year_max", self.pub_year_max.map(|v| v.to_string())),
                ("pub_year_min", self.pub_year_min.map(|v| v.to_string())),
                (
                    "recency_minutes",
                    self.recency_minutes.map(|v| v.to_string()),
                ),
            ] {
                if let Some(value) = value {
                    params.append_pair(name, &value);
                }
            }
        }
        Ok(url)
    }
}

#[derive(Serialize)]
#[serde(crate = "berm_lang::serde")]
pub(crate) struct FetchRequest<'a> {
    urls: &'a [String],
    #[serde(flatten)]
    options: &'a FetchOptions,
}

impl<'a> TryFrom<(&'a [String], &'a FetchOptions)> for FetchRequest<'a> {
    type Error = Error;

    fn try_from((urls, options): (&'a [String], &'a FetchOptions)) -> Result<Self> {
        if !(1..=10).contains(&urls.len()) {
            return Err("Provide 1 to 10 URLs".into());
        }
        for value in urls {
            check_url(value)?;
        }
        if urls.len() != 1
            && (options.if_none_match.is_some() || options.if_modified_since.is_some())
        {
            return Err("Conditional fetching accepts exactly one URL".into());
        }
        if options
            .per_url_timeout_ms
            .is_some_and(|timeout| !(1..=110000).contains(&timeout))
        {
            return Err("Fetch timeout must be from 1 to 110000 milliseconds".into());
        }
        Ok(Self { urls, options })
    }
}

impl AgentConfig {
    pub(crate) fn validate(&self) -> Result<()> {
        if self
            .max_steps
            .is_some_and(|steps| !(1..=500).contains(&steps))
        {
            return Err("Agent max_steps must be from 1 to 500".into());
        }
        if self.max_duration_seconds == Some(0) {
            return Err("Agent duration must be positive".into());
        }
        if self
            .cursor_style
            .as_deref()
            .is_some_and(|s| !matches!(s, "fish" | "standard" | "green-pointer"))
        {
            return Err("Unsupported TinyFish cursor style".into());
        }
        Ok(())
    }
}

#[derive(Serialize)]
#[serde(crate = "berm_lang::serde")]
pub(crate) struct AgentRequest<'a> {
    url: &'a str,
    goal: &'a str,
    #[serde(flatten)]
    options: &'a AgentOptions,
}

impl<'a> TryFrom<(&'a str, &'a str, &'a AgentOptions)> for AgentRequest<'a> {
    type Error = Error;

    fn try_from((url, goal, options): (&'a str, &'a str, &'a AgentOptions)) -> Result<Self> {
        check_url(url)?;
        if goal.trim().is_empty() {
            return Err("Provide a browser-agent goal".into());
        }
        if let Some(config) = &options.agent_config {
            config.validate()?;
        }
        Ok(Self { url, goal, options })
    }
}

#[derive(Deserialize)]
#[serde(crate = "berm_lang::serde")]
pub(crate) struct StartResponse {
    run_id: Option<String>,
    error: Option<Value>,
}

impl TryFrom<StartResponse> for StartedRun {
    type Error = Error;

    fn try_from(response: StartResponse) -> Result<Self> {
        if let Some(error) = response.error {
            return Err(format!("TinyFish agent: {error}").into());
        }
        let run_id = response
            .run_id
            .filter(|id| !id.is_empty())
            .ok_or("TinyFish returned no run ID")?;
        Ok(Self { run_id })
    }
}

fn check_url(value: &str) -> Result<()> {
    let url = Url::parse(value).map_err(|_| "Provide an HTTP or HTTPS URL")?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        return Err("Provide an HTTP or HTTPS URL".into());
    }
    Ok(())
}
