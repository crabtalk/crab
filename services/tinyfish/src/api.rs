use crate::{
    bindings::{
        agent, fetch,
        search::{self, types::SearchInput},
    },
    types::{
        AgentOptions, AgentRequest, CancelledRun, FetchOptions, FetchRequest, FetchResult,
        Identifier, Run, Screenshot, SearchOptions, SearchPage, StartResponse, StartedRun,
    },
};
use alloc::string::String;
use base64::{Engine, engine::general_purpose};
use berm_lang::{
    Result,
    serde_json::{self, json},
};

const WEB: &str = "web";
const NEWS: &str = "news";
const PUBLICATIONS: &str = "research_paper";
impl SearchOptions {
    fn search(&self, query: &str, domain: &str) -> Result<SearchPage> {
        search::api::search(&SearchInput::try_from((query, domain, self))?)?.read("TinyFish")
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
    let input = serde_json::from_value(json!({"body": request}))?;
    fetch::api::fetch(&input)?.read("TinyFish")
}

/// The returned ID names an asynchronous run; starting does not imply completion.
pub fn run(url: &str, goal: &str, options: &AgentOptions) -> Result<StartedRun> {
    let request = AgentRequest::try_from((url, goal, options))?;
    let input = serde_json::from_value(json!({"body": request}))?;
    let response: StartResponse = agent::api::run(&input)?.read("TinyFish")?;
    response.try_into()
}

pub fn run_status(run_id: &str) -> Result<Run> {
    let input = serde_json::from_value(json!({"path": {"id": Identifier::try_from(run_id)?.0}}))?;
    agent::api::run_status(&input)?.read("TinyFish")
}

/// Completed upstream steps remain billable after cancellation.
pub fn run_cancel(run_id: &str) -> Result<CancelledRun> {
    let input = serde_json::from_value(json!({"path": {"id": Identifier::try_from(run_id)?.0}}))?;
    agent::api::run_cancel(&input)?.read("TinyFish")
}

/// Screenshots must have been enabled when starting the run.
pub fn step_screenshot(run_id: &str, step_id: &str) -> Result<Screenshot> {
    let input = serde_json::from_value(
        json!({"path": {"id": Identifier::try_from(run_id)?.0, "stepId": Identifier::try_from(step_id)?.0}}),
    )?;
    let response = agent::api::step_screenshot(&input)?;
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
    let input = serde_json::from_value(
        json!({"path": {"id": Identifier::try_from(run_id)?.0, "stepId": Identifier::try_from(step_id)?.0}}),
    )?;
    let response = agent::api::step_html(&input)?;
    String::from_utf8(response.body).map_err(|_| "TinyFish returned invalid UTF-8 HTML".into())
}
