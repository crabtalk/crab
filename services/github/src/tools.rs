use crate::{
    api,
    types::{
        Comment, Issue, Issues, NewIssue, NewPullRequest, NewRelease, Page, PullRequest,
        PullRequests, Release, Releases, Repository,
    },
};
use alloc::string::String;
use berm_lang::{Result, tool};

/// Read repository metadata.
#[tool]
pub fn repository(
    /// Repository owner or organization.
    owner: String,
    /// Repository name without .git.
    repo: String,
) -> Result<Repository> {
    api::repository(&owner, &repo)
}

/// List repository issues, including pull requests. Filter entries with pull_request when only issues are wanted.
#[tool]
pub fn issues(
    /// Repository owner or organization.
    owner: String,
    /// Repository name without .git.
    repo: String,
    /// open, closed, or all; open by default.
    state: Option<String>,
    /// Page number and size; defaults to page 1 with 30 items.
    page: Option<Page>,
) -> Result<Issues> {
    api::issues(&owner, &repo, state.as_deref(), &page.unwrap_or_default())
}

/// Read an issue.
#[tool]
pub fn issue(
    /// Repository owner or organization.
    owner: String,
    /// Repository name without .git.
    repo: String,
    /// Repository-local issue number.
    number: u64,
) -> Result<Issue> {
    api::issue(&owner, &repo, number)
}

/// Create an issue in the connected GitHub account.
#[tool]
pub fn create_issue(
    /// Repository owner or organization.
    owner: String,
    /// Repository name without .git.
    repo: String,
    /// Title, optional description and labels.
    issue: NewIssue,
) -> Result<Issue> {
    api::create_issue(&owner, &repo, &issue)
}

/// Add a comment to an issue or pull request.
#[tool]
pub fn comment(
    /// Repository owner or organization.
    owner: String,
    /// Repository name without .git.
    repo: String,
    /// Repository-local issue or pull-request number.
    number: u64,
    /// Comment in GitHub Markdown.
    body: String,
) -> Result<Comment> {
    api::comment(&owner, &repo, number, &body)
}

/// List pull requests.
#[tool]
pub fn pull_requests(
    /// Repository owner or organization.
    owner: String,
    /// Repository name without .git.
    repo: String,
    /// open, closed, or all; open by default.
    state: Option<String>,
    /// Page number and size.
    page: Option<Page>,
) -> Result<PullRequests> {
    api::pull_requests(&owner, &repo, state.as_deref(), &page.unwrap_or_default())
}

/// Read a pull request.
#[tool]
pub fn pull_request(
    /// Repository owner or organization.
    owner: String,
    /// Repository name without .git.
    repo: String,
    /// Repository-local pull-request number.
    number: u64,
) -> Result<PullRequest> {
    api::pull_request(&owner, &repo, number)
}

/// Open a pull request between existing branches.
#[tool]
pub fn create_pull_request(
    /// Repository owner or organization.
    owner: String,
    /// Repository name without .git.
    repo: String,
    /// Title, head branch, base branch and optional body or draft flag.
    pull: NewPullRequest,
) -> Result<PullRequest> {
    api::create_pull_request(&owner, &repo, &pull)
}

/// List releases, excluding tags without a release.
#[tool]
pub fn releases(
    /// Repository owner or organization.
    owner: String,
    /// Repository name without .git.
    repo: String,
    /// Page number and size.
    page: Option<Page>,
) -> Result<Releases> {
    api::releases(&owner, &repo, &page.unwrap_or_default())
}

/// Read the latest non-draft, non-prerelease release.
#[tool]
pub fn latest_release(
    /// Repository owner or organization.
    owner: String,
    /// Repository name without .git.
    repo: String,
) -> Result<Release> {
    api::latest_release(&owner, &repo)
}

/// Create a release; this can also create a Git tag.
#[tool]
pub fn create_release(
    /// Repository owner or organization.
    owner: String,
    /// Repository name without .git.
    repo: String,
    /// Tag and optional target, title, notes, draft or prerelease flag.
    release: NewRelease,
) -> Result<Release> {
    api::create_release(&owner, &repo, &release)
}
