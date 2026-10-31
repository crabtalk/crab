use alloc::{string::String, vec::Vec};
use berm_lang::Schema;

/// Repository metadata.
#[derive(Debug, Clone, Schema)]
pub struct Repository {
    pub full_name: String,
    pub html_url: String,
    pub description: Option<String>,
    pub default_branch: String,
    pub private: bool,
}

/// An issue, or a pull request returned by GitHub's issues endpoint.
#[derive(Debug, Clone, Schema)]
pub struct Issue {
    pub number: u64,
    pub title: String,
    pub body: Option<String>,
    pub state: String,
    pub html_url: String,
    pub pull_request: Option<PullLink>,
}

/// Present when an issues-list entry is a pull request.
#[derive(Debug, Clone, Schema)]
pub struct PullLink {
    pub html_url: String,
}

/// A pull request.
#[derive(Debug, Clone, Schema)]
pub struct PullRequest {
    pub number: u64,
    pub title: String,
    pub body: Option<String>,
    pub state: String,
    pub html_url: String,
    pub draft: bool,
}

/// A published or draft release; tags without releases are not included.
#[derive(Debug, Clone, Schema)]
pub struct Release {
    pub id: u64,
    pub tag_name: String,
    pub name: Option<String>,
    pub body: Option<String>,
    pub html_url: String,
    pub draft: bool,
    pub prerelease: bool,
    pub published_at: Option<String>,
}

/// A comment on an issue or pull request.
#[derive(Debug, Clone, Schema)]
pub struct Comment {
    pub id: u64,
    pub body: String,
    pub html_url: String,
}

/// Fields for a new issue.
#[derive(Debug, Clone, Schema)]
pub struct NewIssue {
    pub title: String,
    pub body: Option<String>,
    pub labels: Option<Vec<String>>,
}

/// Fields for a new pull request. Head can be owner:branch for a fork.
#[derive(Debug, Clone, Schema)]
pub struct NewPullRequest {
    pub title: String,
    pub head: String,
    pub base: String,
    pub body: Option<String>,
    pub draft: Option<bool>,
}

/// Fields for a new release. Creating a release may create its tag.
#[derive(Debug, Clone, Schema)]
pub struct NewRelease {
    pub tag_name: String,
    pub target_commitish: Option<String>,
    pub name: Option<String>,
    pub body: Option<String>,
    pub draft: Option<bool>,
    pub prerelease: Option<bool>,
}

/// Listing parameters: 30 items and page 1 by default; at most 100 items.
#[derive(Debug, Clone, Default, Schema)]
pub struct Page {
    pub page: Option<u64>,
    pub per_page: Option<u64>,
}

/// An issues-list page, including pull requests as GitHub returns them.
#[derive(Debug, Clone, Schema)]
pub struct Issues {
    pub items: Vec<Issue>,
    pub next_page: Option<u64>,
}

/// A pull-request page.
#[derive(Debug, Clone, Schema)]
pub struct PullRequests {
    pub items: Vec<PullRequest>,
    pub next_page: Option<u64>,
}

/// A release page.
#[derive(Debug, Clone, Schema)]
pub struct Releases {
    pub items: Vec<Release>,
    pub next_page: Option<u64>,
}

/// The authenticated GitHub user's public profile.
#[derive(Debug, Clone, Schema)]
pub struct User {
    pub id: u64,
    pub login: String,
    pub name: Option<String>,
    pub email: Option<String>,
    pub avatar_url: Option<String>,
}
