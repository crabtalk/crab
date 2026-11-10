use anyhow::Result;
use berm_api::Output;
use berm_build::fixture;
use berm_host::test::TestHost;
use serde_json::json;

#[tokio::test]
async fn secret_resources_use_service_and_host_grants_without_declaring_each_id() -> Result<()> {
    let host = TestHost::open(0).await?;
    let image = fixture::image("berm-fixture")?;
    host.deploy("relay", &image).await?;
    host.deploy("other", &image).await?;
    let id = "secret_11111111111111111111111111111111";
    let url = "http://127.0.0.1/routine";
    let input = json!({"url":url,"header":format!("Authorization: Bearer {{secret:{id}}}")});
    host.arrange("GET", url, 200, "ok");
    assert!(matches!(
        host.call("relay", "fetch", input.to_string().as_bytes())
            .await?
            .result,
        Output::Failed(_)
    ));
    assert!(host.fetched().is_empty());
    host.grant("relay", id, "127.0.0.1", "first-key");
    host.call_json("relay", "fetch", input.clone()).await?;
    assert_eq!(
        host.fetched()[0].headers,
        [("Authorization".into(), "Bearer first-key".into())]
    );
    host.grant("relay", id, "127.0.0.1", "rotated-key");
    host.call_json("relay", "fetch", input.clone()).await?;
    assert_eq!(
        host.fetched()[1].headers,
        [("Authorization".into(), "Bearer rotated-key".into())]
    );
    assert!(matches!(
        host.call("other", "fetch", input.to_string().as_bytes())
            .await?
            .result,
        Output::Failed(_)
    ));
    let missing = input
        .to_string()
        .replace(id, "secret_22222222222222222222222222222222");
    assert!(matches!(
        host.call("relay", "fetch", missing.as_bytes())
            .await?
            .result,
        Output::Failed(_)
    ));
    assert_eq!(host.fetched().len(), 2);
    Ok(())
}
