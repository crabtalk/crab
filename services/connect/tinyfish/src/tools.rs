use crate::{
    api,
    types::{
        AgentOptions, CancelledRun, FetchOptions, FetchResult, Run, Screenshot, SearchOptions,
        SearchPage, StartedRun,
    },
};
use alloc::{string::String, vec::Vec};
use berm_lang::{Result, tool};

/// Search the web
///
/// Search one page of the open web through the deployment's TinyFish account.
#[tool(catalog = "web.search")]
pub fn search(
    /// Search query.
    query: String,
    /// Pagination, domains, dates, language, location and optional result fetching.
    options: Option<SearchOptions>,
) -> Result<SearchPage> {
    api::search(&query, &options.unwrap_or_default())
}

/// Search recent news
///
/// Search one page of recent news through TinyFish.
#[tool(catalog = "web.search.news")]
pub fn search_news(
    /// News query.
    query: String,
    /// Pagination, freshness, domains, language and location.
    options: Option<SearchOptions>,
) -> Result<SearchPage> {
    api::search_news(&query, &options.unwrap_or_default())
}

/// Search research publications
///
/// Search academic publications with year filters and citation metadata.
#[tool(catalog = "web.search.publications")]
pub fn search_publications(
    /// Research topic.
    query: String,
    /// Pagination, publication years, domains and language.
    options: Option<SearchOptions>,
) -> Result<SearchPage> {
    api::search_publications(&query, &options.unwrap_or_default())
}

/// Fetch clean page content
///
/// Extract 1–10 URLs as Markdown, HTML or JSON. Results retain per-URL failures.
#[tool(catalog = "web.extract")]
pub fn fetch(
    /// HTTP or HTTPS URLs; at most ten.
    urls: Vec<String>,
    /// Output format, links, metadata, selectors, caching and conditional request options.
    options: Option<FetchOptions>,
) -> Result<FetchResult> {
    api::fetch(&urls, &options.unwrap_or_default())
}

/// Run a browser agent
///
/// Start an asynchronous browser agent to work toward a natural-language goal.
#[tool(catalog = "web.agent.run")]
pub fn run(
    /// Starting HTTP or HTTPS page URL.
    url: String,
    /// What the browser agent should accomplish.
    goal: String,
    /// Browser profile, step limits, capture, proxy and structured output.
    options: Option<AgentOptions>,
) -> Result<StartedRun> {
    api::run(&url, &goal, &options.unwrap_or_default())
}

/// Check a browser-agent run
///
/// Read an agent run's status, result, errors and captured step metadata.
#[tool(catalog = "web.agent.run.status")]
pub fn run_status(
    /// Identifier returned by run.
    run_id: String,
) -> Result<Run> {
    api::run_status(&run_id)
}

/// Cancel a browser-agent run
///
/// Cancel a browser-agent run. Completed upstream steps remain billable.
#[tool(catalog = "web.agent.run.cancel")]
pub fn run_cancel(
    /// Identifier returned by run.
    run_id: String,
) -> Result<CancelledRun> {
    api::run_cancel(&run_id)
}

/// Retrieve a step screenshot
///
/// Retrieve a captured step screenshot as base64-encoded JPEG.
#[tool(catalog = "web.agent.step.screenshot")]
pub fn step_screenshot(
    /// Identifier returned by run.
    run_id: String,
    /// Step identifier from run_status; screenshots must have been enabled.
    step_id: String,
) -> Result<Screenshot> {
    api::step_screenshot(&run_id, &step_id)
}

/// Retrieve step HTML
///
/// Retrieve a captured step's HTML snapshot.
#[tool(catalog = "web.agent.step.html")]
pub fn step_html(
    /// Identifier returned by run.
    run_id: String,
    /// Step identifier from run_status; HTML capture must have been enabled.
    step_id: String,
) -> Result<String> {
    api::step_html(&run_id, &step_id)
}
