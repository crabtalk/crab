use crate::types::{
    AgentOptions, AgentRequest, CancelledRun, FetchOptions, FetchRequest, FetchResult, Run,
    Screenshot, SearchOptions, SearchPage, StartResponse, StartedRun,
};
use alloc::string::String;
use base64::{Engine, engine::general_purpose};
use berm_lang::{Result, http::Client, serde_json};
use url::Url;

const WEB: &str = "web";
const NEWS: &str = "news";
const PUBLICATIONS: &str = "research_paper";
const CANCEL: &str = "cancel";
const STEPS: &str = "steps";
const SCREENSHOT: &str = "screenshot";
const HTML: &str = "html";
const SEARCH_API: &str = "https://api.search.tinyfish.ai";
const FETCH_API: &str = "https://api.fetch.tinyfish.ai";
const AGENT_API: &str = "https://agent.tinyfish.ai/v1/automation/run-async";
const RUNS_API: &str = "https://agent.tinyfish.ai/v1/runs";
const CLIENT: Client<'static> = Client {
    name: "TinyFish",
    headers: &[
        ("X-API-Key", "{secret:TINYFISH_API_KEY}"),
        ("Content-Type", "application/json"),
    ],
};

impl SearchOptions {
    fn search(&self, query: &str, domain: &str) -> Result<SearchPage> {
        let url = self.url(SEARCH_API, query, domain)?;
        CLIENT.json("GET", url.as_str(), &[])
    }
}

/// Request subsequent pages explicitly.
pub fn search(query: &str, options: &SearchOptions) -> Result<SearchPage> {
    options.search(query, WEB)
}

pub fn search_news(query: &str, options: &SearchOptions) -> Result<SearchPage> {
    options.search(query, NEWS)
}

pub fn search_publications(query: &str, options: &SearchOptions) -> Result<SearchPage> {
    options.search(query, PUBLICATIONS)
}

/// Inspect both results and errors for partial URL failures.
pub fn fetch(urls: &[String], options: &FetchOptions) -> Result<FetchResult> {
    let request = FetchRequest::try_from((urls, options))?;
    CLIENT.json("POST", FETCH_API, &serde_json::to_vec(&request)?)
}

/// The returned ID names an asynchronous run; starting does not imply completion.
pub fn run(url: &str, goal: &str, options: &AgentOptions) -> Result<StartedRun> {
    let request = AgentRequest::try_from((url, goal, options))?;
    let response: StartResponse = CLIENT.json("POST", AGENT_API, &serde_json::to_vec(&request)?)?;
    response.try_into()
}

fn run_url(run_id: &str, suffix: &[&str]) -> Result<Url> {
    if core::iter::once(run_id)
        .chain(suffix.iter().copied())
        .any(|id| id.is_empty() || matches!(id, "." | ".."))
    {
        return Err("Provide nonempty run and step identifiers, without dot segments".into());
    }
    let mut url = Url::parse(RUNS_API).expect("constant URL");
    url.path_segments_mut()
        .expect("HTTP URL")
        .push(run_id)
        .extend(suffix.iter().copied());
    Ok(url)
}

pub fn run_status(run_id: &str) -> Result<Run> {
    CLIENT.json("GET", run_url(run_id, &[])?.as_str(), &[])
}

/// Completed upstream steps remain billable after cancellation.
pub fn run_cancel(run_id: &str) -> Result<CancelledRun> {
    CLIENT.json("POST", run_url(run_id, &[CANCEL])?.as_str(), &[])
}

/// Screenshots must have been enabled when starting the run.
pub fn step_screenshot(run_id: &str, step_id: &str) -> Result<Screenshot> {
    let response = CLIENT.request(
        "GET",
        run_url(run_id, &[STEPS, step_id, SCREENSHOT])?.as_str(),
        &[],
    )?;
    Ok(Screenshot {
        content_type: response
            .header("content-type")
            .unwrap_or("image/jpeg")
            .into(),
        base64: general_purpose::STANDARD.encode(response.body),
    })
}

/// HTML capture must have been enabled when starting the run.
pub fn step_html(run_id: &str, step_id: &str) -> Result<String> {
    let response = CLIENT.request(
        "GET",
        run_url(run_id, &[STEPS, step_id, HTML])?.as_str(),
        &[],
    )?;
    String::from_utf8(response.body).map_err(|_| "TinyFish returned invalid UTF-8 HTML".into())
}
