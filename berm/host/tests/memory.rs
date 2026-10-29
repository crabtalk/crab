//! The memory a scope's services hold, each and together.

use anyhow::Result;
use berm_host::test::{self, TestHost};

/// 2026-10-12T00:00:00Z.
const NOW: u64 = 1_791_763_200_000;

#[tokio::test]
async fn a_scope_answers_each_service_it_holds() -> Result<()> {
    let host = TestHost::open(NOW).await?;
    host.deploy("acct/fixture", &test::build("berm-fixture")?)
        .await?;

    let memory = host.host.memory("acct");
    let held = memory
        .services
        .get("fixture")
        .expect("the fixture's memory");
    assert!(held.loaded > 0);
    assert_eq!(held.running, 0);
    assert_eq!(memory.loaded, held.loaded);

    assert!(host.host.memory("other").services.is_empty());
    Ok(())
}
