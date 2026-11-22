use anyhow::Result;
use axum::{
    Extension,
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use berm_api::host::Caller;
use berm_build::fixture;
use berm_host::{Layers, Named, Outside, deny, router, test::TestHost};
use serde_json::json;
use std::sync::{Arc, Mutex};
use tower::{ServiceExt, layer::layer_fn, service_fn};

#[tokio::test]
async fn conditional_queries_use_the_same_tool_admission_as_other_queries() -> Result<()> {
    let refused = Arc::new(Mutex::new(None));
    let gate = refused.clone();
    let host = TestHost::open_with_layers(
        0,
        Layers::new(layer_fn(move |inner: Outside| {
            let gate = gate.clone();
            Outside::new(service_fn(move |call| {
                let refused = *gate.lock().unwrap();
                let inner = inner.clone();
                async move {
                    if let Some(status) = refused {
                        return Err(deny(status, "admission refused"));
                    }
                    inner.oneshot(call).await
                }
            }))
        })),
    )
    .await?;
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
    let cached = router
        .clone()
        .oneshot(request("peek", Some(tag.clone()), true))
        .await?;
    assert_eq!(cached.status(), StatusCode::NOT_MODIFIED);
    assert_eq!(cached.headers()[header::ETAG], tag);
    assert!(!cached.headers().contains_key(berm_api::host::METER));
    assert!(to_bytes(cached.into_body(), 1024).await?.is_empty());

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

    for refusal in [StatusCode::PAYMENT_REQUIRED, StatusCode::TOO_MANY_REQUESTS] {
        *refused.lock().unwrap() = Some(refusal);
        for condition in [None, Some(tag.clone())] {
            let response = router
                .clone()
                .oneshot(request("peek", condition, true))
                .await?;
            assert_eq!(response.status(), refusal);
        }
    }
    *refused.lock().unwrap() = None;

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
