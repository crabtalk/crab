//! Syscalls the manifest does not declare, refused by the host.

use anyhow::Result;
use berm_api::Output;
use berm_host::test::{self, TestHost};

#[tokio::test]
async fn a_fetch_is_refused_without_the_declaration() -> Result<()> {
    let host = TestHost::open(0).await?;
    host.deploy("bare", &test::build("berm-bare")?).await?;
    let Output::Failed(failure) = host.call("bare", "fetch", b"{}").await?.result else {
        panic!("fetch is refused");
    };
    assert!(
        failure.contains("does not declare example.com"),
        "{failure}"
    );
    Ok(())
}

#[tokio::test]
async fn vouching_is_refused_without_the_declaration() -> Result<()> {
    let host = TestHost::open(0).await?;
    host.deploy("bare", &test::build("berm-bare")?).await?;
    let Output::Failed(failure) = host.call("bare", "vouch", b"{}").await?.result else {
        panic!("vouch is refused");
    };
    assert!(failure.contains("does not declare berm.vouch"), "{failure}");
    Ok(())
}
