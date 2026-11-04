//! HTTP needs destinations; context and vouching need no declarations.

use anyhow::Result;
use berm_api::{Manifest, Output};
use berm_build::fixture;
use berm_host::test::TestHost;

#[tokio::test]
async fn a_fetch_is_refused_without_destinations() -> Result<()> {
    let host = TestHost::open(0).await?;
    host.deploy("bare", &fixture::image("berm-bare")?).await?;
    let Output::Failed(failure) = host.call("bare", "fetch", b"{}").await?.result else {
        panic!("fetch is refused");
    };
    assert!(
        failure.contains("does not declare example.com"),
        "{failure}"
    );
    assert!(host.fetched().is_empty());
    Ok(())
}

#[tokio::test]
async fn vouching_needs_no_declaration() -> Result<()> {
    let host = TestHost::open(0).await?;
    host.deploy("bare", &fixture::image("berm-bare")?).await?;
    let ran = host.call("bare", "vouch", b"{}").await?;
    assert!(matches!(ran.result, Output::Done(_)), "{:?}", ran.result);
    Ok(())
}

#[tokio::test]
async fn client_ip_needs_no_declaration() -> Result<()> {
    let host = TestHost::open(0).await?;
    host.deploy("bare", &fixture::image("berm-bare")?).await?;
    let ran = host.call("bare", "client_ip", b"{}").await?;
    assert!(matches!(ran.result, Output::Done(_)), "{:?}", ran.result);
    Ok(())
}

#[test]
fn the_linked_imports_identify_calls_without_declarations() -> Result<()> {
    let manifest = Manifest::try_from(fixture::image("berm-bare")?.as_slice())?;
    for name in ["berm.http.fetch", "berm.vouch", "berm.client.ip"] {
        assert!(
            manifest.imports.iter().any(|import| import == name),
            "{name}"
        );
    }
    assert!(manifest.deps.http.is_empty());
    Ok(())
}
