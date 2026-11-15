use anyhow::Result;
use axum::{body::to_bytes, http::StatusCode, response::IntoResponse};
use berm_api::workflow::config::Invalid;
use berm_build::fixture;
use berm_host::test::TestHost;
use serde_json::{Value, json};
#[path = "../../api/tests/support/workflow.rs"]
mod workflow;

#[tokio::test]
async fn workflow_only_services_deploy_with_public_and_owner_metadata() -> Result<()> {
    let host = TestHost::open(0).await?;
    let image = workflow::image(
        json!({"access":"public","price":1000,"max_cost":2000,"checkpoint_bytes":4096}),
    );
    host.deploy("identity", &image).await?;
    let public = serde_json::to_value(host.host.idl("identity")?)?;
    assert_eq!(public["workflows"][0]["price"], 1000);
    assert!(public["workflows"][0].get("max_cost").is_none());
    assert!(public["workflows"][0].get("config").is_none());
    let owner = host.host.runtime("identity").await?;
    assert_eq!(owner.workflows[0].config.max_cost, 2000);
    let internal = workflow::image(json!({"max_cost":2000,"checkpoint_bytes":4096}));
    host.deploy("internal", &internal).await?;
    assert!(host.host.idl("internal")?.workflows.is_empty());
    assert_eq!(host.host.runtime("internal").await?.workflows.len(), 1);
    host.deploy("mixed", &fixture::image("berm-fixture")?)
        .await?;
    assert!(!host.host.idl("mixed")?.workflows.is_empty());
    Ok(())
}

#[tokio::test]
async fn invalid_workflow_config_reaches_the_caller_with_its_explanation() -> Result<()> {
    let host = TestHost::open(0).await?;
    let image = workflow::image(
        json!({"max_cost":1000,"checkpoint_bytes":4096,"schedule":{"cron":"0 8 * * *","input":"wrong"}}),
    );
    let error = match host.deploy("identity", &image).await {
        Ok(_) => panic!("accepted invalid schedule input"),
        Err(error) => error,
    };
    let response = error.downcast::<berm_host::Denied>()?.into_response();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body: Value = serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await?)?;
    assert_eq!(body["explain"], Invalid::EXPLAIN);
    Ok(())
}
