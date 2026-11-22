use anyhow::Result;
use axum::{
    Extension,
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use berm_api::Manifest;
use berm_build::fixture;
use berm_host::{
    Named, router,
    test::{TestHost, image},
};
use serde_json::{Value, json};
use tower::ServiceExt;

#[tokio::test]
async fn live_settings_survive_redeploy_until_the_declaration_is_removed() -> Result<()> {
    let host = TestHost::open(0).await?;
    let original = fixture::image("berm-fixture")?;
    host.deploy("settings", &original).await?;
    let api = router(host.host.clone()).layer(Extension(Named("settings".into())));
    let put = |name: &str, value: Value| {
        Request::put(format!("/vars/{name}"))
            .header("content-type", "application/json")
            .body(Body::from(json!({"value":value}).to_string()))
            .unwrap()
    };
    assert_eq!(
        host.call_json("settings", "greet", json!({})).await?,
        "hello"
    );
    for (name, value, status) in [
        ("missing", json!("no"), StatusCode::NOT_FOUND),
        ("greeting", json!("a".repeat(4097)), StatusCode::BAD_REQUEST),
    ] {
        let response = api.clone().oneshot(put(name, value)).await?;
        assert_eq!(response.status(), status);
        let body: Value = serde_json::from_slice(&to_bytes(response.into_body(), 1024).await?)?;
        assert_eq!(body["explain"], "E0011");
    }
    assert_eq!(
        api.oneshot(put("greeting", json!("live"))).await?.status(),
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        host.call_json("settings", "greet", json!({})).await?,
        "live"
    );
    host.deploy("settings", &original).await?;
    assert_eq!(
        host.call_json("settings", "greet", json!({})).await?,
        "live"
    );

    let mut deps = Manifest::try_from(original.as_slice())?.deps;
    deps.vars.clear();
    let without_var = image::with_deps(&original, deps)?;
    host.deploy("settings", &without_var).await?;
    host.deploy("settings", &original).await?;
    assert_eq!(
        host.call_json("settings", "greet", json!({})).await?,
        "hello"
    );
    Ok(())
}
