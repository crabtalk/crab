use anyhow::Result;
use berm_api::Output;
use berm_build::fixture;
use berm_host::test::TestHost;
use serde_json::json;

#[tokio::test]
async fn calls_use_deployment_addresses_without_an_allowlist() -> Result<()> {
    let host = TestHost::open(0).await?;
    let image = fixture::image("berm-fixture")?;
    host.deploy("alice/a", &image).await?;
    host.deploy("alice/b", &image).await?;
    for target in ["b", "alice/b"] {
        let args = json!({"service":target,"tool":"whoami","args":"{}"}).to_string();
        let ran = host.call("alice/a", "invoke", args.as_bytes()).await?;
        assert_eq!(
            ran.result,
            Output::Done(serde_json::to_string(&serde_json::to_string(
                "service alice/a"
            )?)?)
        );
    }
    host.deploy("bob/b", &image).await?;
    let args = json!({"service":"bob/b","tool":"whoami","args":"{}"}).to_string();
    let ran = host.call("alice/a", "invoke", args.as_bytes()).await?;
    assert!(matches!(ran.result, Output::Failed(message) if message.contains("account gateway")));
    Ok(())
}
