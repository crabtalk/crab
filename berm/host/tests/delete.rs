use anyhow::Result;
use berm_build::fixture;
use berm_host::test::TestHost;
use serde_json::json;

#[tokio::test]
async fn delete_removes_a_key_within_the_call_and_once_committed() -> Result<()> {
    let host = TestHost::open(0).await?;
    host.deploy("state", &fixture::image("berm-fixture")?)
        .await?;
    host.deploy("keeper", &fixture::image("berm-keeper")?)
        .await?;
    host.call_json("state", "stored_text", json!({"key":"kept","text":"x"}))
        .await?;
    host.call("keeper", "put", br#"{"word":"x"}"#).await?;
    for (service, key) in [("state", "kept"), ("keeper", "last")] {
        assert_eq!(
            host.call_json(service, "forget", json!({"key":key}))
                .await?,
            json!({"before":true,"after":false}),
            "{service}"
        );
        assert_eq!(
            host.call_json(service, "forget", json!({"key":key}))
                .await?,
            json!({"before":false,"after":false}),
            "{service}"
        );
    }
    Ok(())
}
