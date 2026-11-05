use anyhow::Result;
use berm::{
    Reach,
    syscall::http::{Request, Response},
};
use berm_host::fixture::Http;
use std::time::Duration;
use tower::ServiceExt;

#[tokio::test]
async fn fixtures_keep_headers_and_apply_the_request_body_limit() -> Result<()> {
    let http = Http::default();
    http.respond(
        "GET",
        "https://example.com",
        Response {
            status: 200,
            headers: vec![("content-type".into(), "text/html".into())],
            body: b"hello".to_vec(),
        },
        Duration::ZERO,
    );
    let request = Request {
        service: "fixture".into(),
        chain: 1,
        span: 0,
        method: "GET".into(),
        url: "https://example.com".into(),
        host: "example.com".into(),
        reach: Reach::Named,
        headers: Vec::new(),
        body: Vec::new(),
        timeout: None,
        limit: Some(3),
    };
    let response = http.clone().oneshot(request).await?;
    assert_eq!(
        response.headers,
        [("content-type".to_owned(), "text/html".to_owned())]
    );
    assert_eq!(response.body, b"hel");
    assert_eq!(http.fetched().len(), 1);
    assert!(http.unanswered().is_empty());
    Ok(())
}

#[tokio::test]
async fn a_delayed_fixture_observes_the_request_timeout() {
    let http = Http::default();
    http.respond(
        "GET",
        "https://example.com",
        Response {
            status: 200,
            headers: Vec::new(),
            body: Vec::new(),
        },
        Duration::from_millis(50),
    );
    let request = Request {
        service: "fixture".into(),
        chain: 1,
        span: 0,
        method: "GET".into(),
        url: "https://example.com".into(),
        host: "example.com".into(),
        reach: Reach::Named,
        headers: Vec::new(),
        body: Vec::new(),
        timeout: Some(Duration::from_millis(1)),
        limit: None,
    };
    assert!(http.oneshot(request).await.is_err());
}
