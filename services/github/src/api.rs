use crate::*;
use alloc::{
    format,
    string::{String, ToString},
    vec::Vec,
};
use berm_lang::{
    Result, http,
    serde::{Serialize, de::DeserializeOwned},
    serde_json,
};

fn request(
    method: &str,
    owner: &str,
    repo: &str,
    path: &[&str],
    query: &[(&str, String)],
    body: &[u8],
) -> Result<http::Response> {
    let mut url = url::Url::parse("https://api.github.com").expect("constant origin");
    url.path_segments_mut()
        .expect("HTTP URL")
        .extend(["repos", owner, repo])
        .extend(path);
    if !query.is_empty() {
        url.query_pairs_mut()
            .extend_pairs(query.iter().map(|(k, v)| (*k, v.as_str())));
    }
    fetch(method, url.as_str(), body, "Bearer {secret:GITHUB_TOKEN}")
}

pub(crate) fn fetch(
    method: &str,
    url: &str,
    body: &[u8],
    authorization: &str,
) -> Result<http::Response> {
    let response = http::fetch(
        method,
        url,
        &[
            ("Authorization", authorization),
            ("Accept", "application/vnd.github+json"),
            ("X-GitHub-Api-Version", "2026-03-10"),
            ("User-Agent", "crabtalk-github"),
            ("Content-Type", "application/json"),
        ],
        body,
    )?;
    if !response.is_success() {
        let value = response.json::<serde_json::Value>().ok();
        let message = value
            .as_ref()
            .and_then(|v| v["message"].as_str())
            .unwrap_or("provider request failed");
        let retry = response
            .header("retry-after")
            .map(|v| format!("; retry-after={v}"))
            .unwrap_or_default();
        let reset = response
            .header("x-ratelimit-reset")
            .map(|v| format!("; rate-limit-reset={v}"))
            .unwrap_or_default();
        return Err(format!("GitHub {}: {message}{retry}{reset}", response.status).into());
    }
    Ok(response)
}
pub(crate) fn read<T: DeserializeOwned>(response: &http::Response) -> Result<T> {
    response
        .json()
        .map_err(|_| String::from("GitHub returned an unreadable response").into())
}
fn get<T: DeserializeOwned>(owner: &str, repo: &str, path: &[&str]) -> Result<T> {
    read(&request("GET", owner, repo, path, &[], &[])?)
}
fn post<T: DeserializeOwned>(
    owner: &str,
    repo: &str,
    path: &[&str],
    body: &impl Serialize,
) -> Result<T> {
    let body = serde_json::to_vec(body).map_err(|e| format!("GitHub request JSON: {e}"))?;
    read(&request("POST", owner, repo, path, &[], &body)?)
}
fn list<T: DeserializeOwned>(
    owner: &str,
    repo: &str,
    path: &[&str],
    page: &Page,
    state: Option<&str>,
) -> Result<(Vec<T>, Option<u64>)> {
    let mut query = Vec::from([
        (
            "per_page",
            page.per_page.unwrap_or(30).clamp(1, 100).to_string(),
        ),
        ("page", page.page.unwrap_or(1).max(1).to_string()),
    ]);
    if let Some(state) = state {
        query.push(("state", state.into()));
    }
    let response = request("GET", owner, repo, path, &query, &[])?;
    let next = response.header("link").and_then(|link| {
        link.split(',').find_map(|part| {
            let (target, relations) = part.trim().split_once('>')?;
            if !relations.split(';').any(|rel| rel.trim() == "rel=\"next\"") {
                return None;
            }
            let url = url::Url::parse(target.strip_prefix('<')?).ok()?;
            url.query_pairs()
                .find_map(|(key, value)| (key == "page").then(|| value.parse().ok()).flatten())
        })
    });
    Ok((read(&response)?, next))
}

/// Read repository metadata.
pub fn repository(owner: &str, repo: &str) -> Result<Repository> {
    get(owner, repo, &[])
}
/// Read open, closed, or all issues. GitHub includes pull requests in this endpoint.
pub fn issues(owner: &str, repo: &str, state: Option<&str>, page: &Page) -> Result<Issues> {
    let (items, next_page) = list(owner, repo, &["issues"], page, state)?;
    Ok(Issues { items, next_page })
}
/// Read an issue by its repository-local number.
pub fn issue(owner: &str, repo: &str, number: u64) -> Result<Issue> {
    get(owner, repo, &["issues", &number.to_string()])
}
/// Create an issue. A failed response is not automatically retried.
pub fn create_issue(owner: &str, repo: &str, issue: &NewIssue) -> Result<Issue> {
    post(owner, repo, &["issues"], issue)
}
/// Comment on an issue or pull request.
pub fn comment(owner: &str, repo: &str, number: u64, body: &str) -> Result<Comment> {
    post(
        owner,
        repo,
        &["issues", &number.to_string(), "comments"],
        &serde_json::json!({"body":body}),
    )
}
/// Read open, closed, or all pull requests.
pub fn pull_requests(
    owner: &str,
    repo: &str,
    state: Option<&str>,
    page: &Page,
) -> Result<PullRequests> {
    let (items, next_page) = list(owner, repo, &["pulls"], page, state)?;
    Ok(PullRequests { items, next_page })
}
/// Read a pull request by its repository-local number.
pub fn pull_request(owner: &str, repo: &str, number: u64) -> Result<PullRequest> {
    get(owner, repo, &["pulls", &number.to_string()])
}
/// Open a pull request between existing branches.
pub fn create_pull_request(owner: &str, repo: &str, pull: &NewPullRequest) -> Result<PullRequest> {
    post(owner, repo, &["pulls"], pull)
}
/// Read releases. Tags without associated releases are excluded.
pub fn releases(owner: &str, repo: &str, page: &Page) -> Result<Releases> {
    let (items, next_page) = list(owner, repo, &["releases"], page, None)?;
    Ok(Releases { items, next_page })
}
/// Read GitHub's latest non-draft, non-prerelease release.
pub fn latest_release(owner: &str, repo: &str) -> Result<Release> {
    get(owner, repo, &["releases", "latest"])
}
/// Create a release, optionally as a draft or prerelease.
pub fn create_release(owner: &str, repo: &str, release: &NewRelease) -> Result<Release> {
    post(owner, repo, &["releases"], release)
}
