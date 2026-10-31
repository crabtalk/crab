use berm_lang::{
    http::Response,
    serde_json::{self, json},
    test,
};
#[cfg(feature = "tools")]
use github::tools;
use github::{
    api,
    types::{NewIssue, NewPullRequest, NewRelease, Page},
};

const ISSUE:&[u8]=br#"{"number":7,"title":"Broken menu","body":null,"state":"open","html_url":"https://github.com/o/r/issues/7"}"#;
const PULL:&[u8]=br#"{"number":8,"title":"Fix menu","body":null,"state":"open","html_url":"https://github.com/o/r/pull/8","draft":false}"#;
const RELEASE:&[u8]=br#"{"id":9,"tag_name":"v1","name":null,"body":null,"html_url":"https://github.com/o/r/releases/v1","draft":false,"prerelease":false,"published_at":"2026-10-31"}"#;

#[test]
fn repository_uses_versioned_api_headers_and_host_managed_authentication() {
    test::respond("GET","https://api.github.com/repos/o/r",200,br#"{"full_name":"o/r","html_url":"https://github.com/o/r","description":null,"default_branch":"main","private":false}"#);
    assert_eq!(api::repository("o", "r").unwrap().full_name, "o/r");
    let request = &test::fetched()[0];
    assert_eq!(
        request.header("Authorization"),
        Some("Bearer {secret:GITHUB_TOKEN}")
    );
    assert_eq!(request.header("X-GitHub-Api-Version"), Some("2026-03-10"));
    assert!(request.header("User-Agent").is_some());
}

#[test]
fn list_preserves_issue_pr_distinction_and_follows_next_page_metadata() {
    let url = "https://api.github.com/repos/o/r/issues?per_page=2&page=1&state=all";
    let body = json!([serde_json::from_slice::<serde_json::Value>(ISSUE).unwrap(),{"number":8,"title":"Fix","body":null,"state":"open","html_url":"https://github.com/o/r/pull/8","pull_request":{"html_url":"https://github.com/o/r/pull/8"}}]);
    test::respond_with("GET",url,Response{status:200,headers:vec![("Link".into(),"<https://api.github.com/repos/o/r/issues?per_page=2&page=2&state=all>; rel=\"next\", <https://api.github.com/repos/o/r/issues?page=5>; rel=\"last\"".into())],body:serde_json::to_vec(&body).unwrap()});
    let page = api::issues(
        "o",
        "r",
        Some("all"),
        &Page {
            page: None,
            per_page: Some(2),
        },
    )
    .unwrap();
    assert_eq!(page.next_page, Some(2));
    assert!(page.items[0].pull_request.is_none());
    assert!(page.items[1].pull_request.is_some());
    test::respond(
        "GET",
        "https://api.github.com/repos/o/r/issues?per_page=2&page=2&state=all",
        200,
        b"[]",
    );
    assert!(
        api::issues(
            "o",
            "r",
            Some("all"),
            &Page {
                page: page.next_page,
                per_page: Some(2)
            }
        )
        .unwrap()
        .next_page
        .is_none()
    );
}

#[test]
fn issue_creation_and_comments_send_only_requested_fields() {
    test::respond(
        "POST",
        "https://api.github.com/repos/o/r/issues",
        201,
        ISSUE,
    );
    let issue = api::create_issue(
        "o",
        "r",
        &NewIssue {
            title: "Broken menu".into(),
            body: None,
            labels: None,
        },
    )
    .unwrap();
    assert_eq!(issue.number, 7);
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&test::fetched()[0].body).unwrap(),
        json!({"title":"Broken menu"})
    );
    test::respond("POST","https://api.github.com/repos/o/r/issues/7/comments",201,br#"{"id":1,"body":"Investigating","html_url":"https://github.com/o/r/issues/7#issuecomment-1"}"#);
    assert_eq!(api::comment("o", "r", 7, "Investigating").unwrap().id, 1);
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&test::fetched()[1].body).unwrap(),
        json!({"body":"Investigating"})
    );
}

#[test]
fn pulls_and_releases_use_their_own_endpoints_and_payloads() {
    test::respond("POST", "https://api.github.com/repos/o/r/pulls", 201, PULL);
    assert_eq!(
        api::create_pull_request(
            "o",
            "r",
            &NewPullRequest {
                title: "Fix menu".into(),
                head: "fix".into(),
                base: "main".into(),
                body: None,
                draft: Some(true)
            }
        )
        .unwrap()
        .number,
        8
    );
    let sent: serde_json::Value = serde_json::from_slice(&test::fetched()[0].body).unwrap();
    assert_eq!(
        sent,
        json!({"title":"Fix menu","head":"fix","base":"main","draft":true})
    );
    test::respond(
        "POST",
        "https://api.github.com/repos/o/r/releases",
        201,
        RELEASE,
    );
    assert_eq!(
        api::create_release(
            "o",
            "r",
            &NewRelease {
                tag_name: "v1".into(),
                target_commitish: None,
                name: None,
                body: Some("Notes".into()),
                draft: Some(true),
                prerelease: None
            }
        )
        .unwrap()
        .tag_name,
        "v1"
    );
    test::respond(
        "GET",
        "https://api.github.com/repos/o/r/releases/latest",
        200,
        RELEASE,
    );
    assert_eq!(api::latest_release("o", "r").unwrap().id, 9);
    test::respond(
        "GET",
        "https://api.github.com/repos/o/r/pulls?per_page=100&page=1",
        200,
        b"[]",
    );
    assert!(
        api::pull_requests(
            "o",
            "r",
            None,
            &Page {
                page: Some(0),
                per_page: Some(200)
            }
        )
        .unwrap()
        .items
        .is_empty()
    );
}

#[test]
fn path_arguments_cannot_change_the_endpoint_or_query() {
    test::respond(
        "GET",
        "https://api.github.com/repos/o%2Fother/r%3Fpage=9/issues/7",
        200,
        ISSUE,
    );
    assert_eq!(api::issue("o/other", "r?page=9", 7).unwrap().number, 7);
}

#[test]
fn rate_limit_errors_preserve_provider_timing_without_retrying_writes() {
    test::respond_with(
        "POST",
        "https://api.github.com/repos/o/r/issues",
        Response {
            status: 403,
            headers: vec![
                ("Retry-After".into(), "60".into()),
                ("X-RateLimit-Reset".into(), "2000".into()),
            ],
            body: br#"{"message":"API rate limit exceeded"}"#.to_vec(),
        },
    );
    let error = api::create_issue(
        "o",
        "r",
        &NewIssue {
            title: "title".into(),
            body: None,
            labels: None,
        },
    )
    .unwrap_err()
    .message;
    assert!(
        error.contains("403")
            && error.contains("retry-after=60")
            && error.contains("rate-limit-reset=2000"),
        "{error}"
    );
    assert_eq!(test::fetched().len(), 1);
}

#[cfg(feature = "tools")]
#[test]
fn tool_dispatch_uses_typed_arguments_and_results() {
    test::respond(
        "GET",
        "https://api.github.com/repos/o/r/releases/latest",
        200,
        RELEASE,
    );
    let out = test::call(
        tools::berm_tool_latest_release,
        br#"{"owner":"o","repo":"r"}"#,
    )
    .unwrap();
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&out).unwrap()["tag_name"],
        "v1"
    );
}
