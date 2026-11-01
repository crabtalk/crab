//! What more than one test of the fixture opens its host with.

use anyhow::Result;
use berm_build::fixture;
use berm_host::test::TestHost;

/// A host with the fixture deployed as each of `services`.
pub async fn deployed(services: &[&str]) -> Result<TestHost> {
    let host = TestHost::open(0).await?;
    let image = fixture::image("berm-fixture")?;
    for service in services {
        host.deploy(service, &image).await?;
    }
    Ok(host)
}
