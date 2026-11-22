use anyhow::Result;
use berm_api::Output;
use berm_build::fixture;
use berm_host::test::TestHost;
use serde_json::json;

#[tokio::test]
async fn an_empty_target_wakes_the_current_deployment_after_restart() -> Result<()> {
    let mut host = TestHost::open(1000).await?;
    host.deploy("acct/renamed", &fixture::image("berm-fixture")?)
        .await?;
    host.call_json(
        "acct/renamed",
        "invoke",
        json!({"service":"","tool":"note","args":"{\"word\":\"kept\"}","delay":0}),
    )
    .await?;
    host.reopen().await?;
    let wakes = host.wakes().await?;
    assert_eq!(wakes.len(), 1);
    assert_eq!(wakes[0].0.calls, "renamed");
    assert_eq!(wakes[0].1, 1000);
    let fired = host.fire().await?;
    assert_eq!(fired.len(), 1);
    assert!(matches!(fired[0].1.result, Output::Done(_)));
    assert_eq!(
        host.state("acct/renamed", "note").await?,
        Some(b"kept".to_vec())
    );
    assert!(host.wakes().await?.is_empty());
    Ok(())
}

#[tokio::test]
async fn a_stopped_target_keeps_its_wake_until_it_starts() -> Result<()> {
    let host = TestHost::open(1000).await?;
    let image = fixture::image("berm-fixture")?;
    host.deploy("acct/armer", &image).await?;
    host.deploy("acct/target", &image).await?;
    host.call_json(
        "acct/armer",
        "invoke",
        json!({"service":"target","tool":"note","args":"{\"word\":\"kept\"}","delay":500}),
    )
    .await?;
    assert!(host.host.stop("acct/target").await?);
    host.advance(500);
    assert!(host.fire().await?.is_empty());
    assert_eq!(host.wakes().await?.len(), 1);
    assert_eq!(host.state("acct/target", "note").await?, None);

    assert!(host.host.start("acct/target").await?);
    let fired = host.fire().await?;
    assert_eq!(fired.len(), 1);
    assert!(matches!(fired[0].1.result, Output::Done(_)));
    assert_eq!(
        host.state("acct/target", "note").await?,
        Some(b"kept".to_vec())
    );
    assert!(host.wakes().await?.is_empty());
    Ok(())
}
