//! What more than one test of the fixture opens its host with.

use anyhow::Result;
use berm_host::test::{self, TestHost};

/// A host with the fixture deployed as each of `services`.
pub async fn deployed(services: &[&str]) -> Result<TestHost> {
    let host = TestHost::open(0).await?;
    let image = test::build("berm-fixture")?;
    for service in services {
        host.deploy(service, &image).await?;
    }
    Ok(host)
}
