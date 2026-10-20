//! Syscalls the manifest does not declare, refused by the host.

use anyhow::Result;
use berm_api::Output;
use berm_host::test::{self, TestHost};

#[tokio::test]
async fn token_syscalls_are_refused_without_the_declaration() -> Result<()> {
    let host = TestHost::open(0).await?;
    host.deploy("bare", &test::build("berm-bare", "bare")?)
        .await?;
    for tool in ["sign", "issuer"] {
        let Output::Failed(failure) = host.call("bare", tool, b"{}").await?.result else {
            panic!("{tool} is refused");
        };
        assert!(
            failure.contains("does not declare berm.token.sign"),
            "{tool}: {failure}"
        );
    }
    Ok(())
}
