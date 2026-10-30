//! Required secrets refuse a deploy before it changes the service.

use anyhow::Result;
use berm_host::test::{self, TestHost};

#[tokio::test]
async fn a_required_secret_unset_refuses_a_deploy() -> Result<()> {
    let host = TestHost::open(0).await?;
    let image = test::build("berm-fixture")?;
    let denied = host
        .deploy_checked("acct/fixture", &image)
        .await
        .err()
        .expect("unset");
    assert_eq!(
        denied
            .downcast_ref::<berm_host::Denied>()
            .expect("deploy refusal")
            .explain,
        Some("E0021")
    );
    assert!(host.host.get("acct/fixture").is_none());
    host.grant("acct/fixture", "fixture", "127.0.0.1", "value");
    host.deploy_checked("acct/fixture", &image).await?;
    assert!(!host.host.stopped("acct/fixture"));
    Ok(())
}

#[tokio::test]
async fn an_optional_secret_unset_lets_a_service_serve() -> Result<()> {
    let host = TestHost::open(0).await?;
    let image = test::build("berm-fixture")?;
    host.grant("acct/fixture", "fixture", "127.0.0.1", "value");
    let _installed = host.deploy_checked("acct/fixture", &image).await?;
    assert!(!host.host.stopped("acct/fixture"));
    Ok(())
}
