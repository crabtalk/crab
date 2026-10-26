//! A scope's memory: its services' images while deployed and its invocations'
//! linear memory while they run, held to its limit.

use anyhow::Result;
use berm_host::test::{self, TestHost};
use std::collections::HashMap;

/// A page of linear memory.
const PAGE: u64 = 64 << 10;

#[tokio::test]
async fn loaded_images_count_and_a_call_gives_back_what_it_held() -> Result<()> {
    let host = TestHost::open(0).await?;
    let program = host
        .deploy("acct/fixture", &test::build("berm-fixture")?)
        .await?;
    host.call("acct/fixture", "carry", b"{}").await?;
    let memory = host.host().memory("acct");
    assert_eq!(memory.limit, 0);
    assert_eq!(memory.loaded, program.bytes());
    assert_eq!(memory.running, 0);
    Ok(())
}

#[tokio::test]
async fn a_call_past_the_limit_fails() -> Result<()> {
    let host = TestHost::open(0).await?;
    let program = host
        .deploy("acct/fixture", &test::build("berm-fixture")?)
        .await?;
    host.host()
        .limit(HashMap::from([("acct".to_owned(), program.bytes() + PAGE)]));
    let failed = match host.call("acct/fixture", "carry", b"{}").await {
        Ok(ran) => format!("{:?}", ran.result),
        Err(e) => format!("{e:#}"),
    };
    assert!(failed.contains("past its limit"), "{failed}");
    assert_eq!(host.host().memory("acct").running, 0);

    host.host().limit(HashMap::new());
    host.call("acct/fixture", "carry", b"{}").await?;
    Ok(())
}

#[tokio::test]
async fn a_deploy_whose_images_would_pass_the_limit_is_refused() -> Result<()> {
    let host = TestHost::open(0).await?;
    let image = test::build("berm-fixture")?;
    let bytes = host.deploy("acct/fixture", &image).await?.bytes();
    host.host()
        .limit(HashMap::from([("acct".to_owned(), bytes + PAGE)]));
    host.deploy("acct/fixture", &image).await?;
    let Err(refused) = host.deploy("acct/inner", &image).await else {
        panic!("two images do not fit");
    };
    assert!(
        format!("{refused:#}").contains("past its limit"),
        "{refused:#}"
    );
    assert_eq!(host.host().memory("acct").loaded, bytes);
    host.deploy("other/fixture", &image).await?;
    Ok(())
}
