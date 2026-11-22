use anyhow::Result;
use axum::{
    Extension,
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use berm_api::host::Caller;
use berm_build::fixture;
use berm_host::{Named, router, test::TestHost};
use serde_json::json;
use tower::ServiceExt;

#[tokio::test]
async fn conditional_queries_use_the_same_tool_admission_as_other_queries() -> Result<()> {
    let host = TestHost::open(0).await?;
    host.deploy("service", &fixture::image("berm-fixture")?)
        .await?;
    let router = router(host.host.clone()).layer(Extension(Named("service".into())));
    let caller: Caller = serde_json::from_value(json!({"id":"alice"}))?;
    let request = |tool: &str, tag, authenticated| {
        let mut request = Request::get(format!("/tools/{tool}?args=%7B%7D"))
            .body(Body::empty())
            .unwrap();
        if let Some(tag) = tag {
            request.headers_mut().insert(header::IF_NONE_MATCH, tag);
        }
        if authenticated {
            request.extensions_mut().insert(caller.clone());
        }
        request
    };

    let first = router.clone().oneshot(request("peek", None, true)).await?;
    assert_eq!(first.status(), StatusCode::OK);
    let tag = first.headers()[header::ETAG].clone();
    assert_eq!(
        router
            .clone()
            .oneshot(request("peek", Some(tag.clone()), true))
            .await?
            .status(),
        StatusCode::NOT_MODIFIED
    );

    for condition in [None, Some(tag.clone())] {
        for (tool, allowed, status) in [
            ("missing", true, StatusCode::NOT_FOUND),
            ("note", true, StatusCode::METHOD_NOT_ALLOWED),
            ("peek", false, StatusCode::UNAUTHORIZED),
        ] {
            let response = router
                .clone()
                .oneshot(request(tool, condition.clone(), allowed))
                .await?;
            assert_eq!(response.status(), status, "{tool}");
        }
    }

    assert!(host.host.stop("service").await?);
    for condition in [None, Some(tag)] {
        let response = router
            .clone()
            .oneshot(request("peek", condition, true))
            .await?;
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        let body: serde_json::Value =
            serde_json::from_slice(&to_bytes(response.into_body(), 1024).await?)?;
        assert_eq!(body["explain"], "E0018");
    }
    Ok(())
}
