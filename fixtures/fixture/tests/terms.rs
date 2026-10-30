//! A tool's terms: the budget its `crab.toml` sets over the host's default,
//! on the fixture's image.

use anyhow::Result;
use berm_api::Budget;
use berm_host::test::{self, TestHost};

#[tokio::test]
async fn a_budget_is_published_in_the_terms() -> Result<()> {
    let host = TestHost::open(0).await?;
    host.deploy("fixture", &test::build("berm-fixture")?)
        .await?;
    let terms = host.host.terms("fixture")?;
    assert_eq!(
        terms.tools["echo"].budget,
        Budget {
            cpu: 10_000,
            read: 10,
            written: 10,
            memory: 16 << 20,
        }
    );
    assert_eq!(
        terms.tools["carry"].budget,
        berm_host::Config::default().budget
    );
    Ok(())
}
