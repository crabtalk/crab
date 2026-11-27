use anyhow::Result;
use axum::{
    Extension,
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use berm::syscall::http::Response;
use berm_api::host::{Busy, Caller};
use berm_build::fixture;
use berm_host::{Config, Named, router, test::TestHost};
use serde_json::{Value, json};
use std::time::Duration;
use tower::ServiceExt;

#[tokio::test]
async fn an_invocation_that_cannot_reserve_its_budget_is_refused_as_node_busy() -> Result<()> {
    let mut config = Config::default();
    config.reserved = config.budget.memory - 1;
    let host = TestHost::open_with(0, config).await?;
    host.deploy("fixture", &fixture::image("berm-fixture")?)
        .await?;
    let caller: Caller = serde_json::from_value(json!({"id":"alice"}))?;
    let response = router(host.host.clone())
        .layer(Extension(Named("fixture".into())))
        .layer(Extension(caller))
        .oneshot(Request::post("/tools/peek").body(Body::from("{}"))?)
        .await?;
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    let body: Value = serde_json::from_slice(&to_bytes(response.into_body(), 1024).await?)?;
    assert_eq!(body["error"], Busy::Memory.message());
    assert_eq!(body["explain"], "E0098");
    assert_eq!(host.host.running(), 0);
    Ok(())
}

#[tokio::test]
async fn a_node_with_no_call_slot_returns_the_capacity_explanation() -> Result<()> {
    let mut config = Config::default();
    config.instances = config.depth + 1;
    let host = TestHost::open_with(0, config).await?;
    host.deploy("fixture", &fixture::image("berm-fixture")?)
        .await?;
    host.fetch.respond(
        "GET",
        "http://127.0.0.1/slow",
        Response {
            status: 200,
            headers: vec![],
            body: b"done".to_vec(),
        },
        Duration::from_secs(1),
    );
    let caller: Caller = serde_json::from_value(json!({"id":"alice"}))?;
    let app = router(host.host.clone())
        .layer(Extension(Named("fixture".into())))
        .layer(Extension(caller));
    let first = tokio::spawn(app.clone().oneshot(
        Request::post("/tools/fetch").body(Body::from(r#"{"url":"http://127.0.0.1/slow"}"#))?,
    ));
    tokio::time::timeout(Duration::from_secs(3), async {
        while host.fetch.fetched().is_empty() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await?;
    let response = app
        .oneshot(Request::post("/tools/peek").body(Body::from("{}"))?)
        .await?;
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    let body: Value = serde_json::from_slice(&to_bytes(response.into_body(), 1024).await?)?;
    assert_eq!(body["error"], Busy::Calls.message());
    assert_eq!(body["explain"], "E0098");
    assert_eq!(first.await??.status(), StatusCode::OK);
    Ok(())
}
