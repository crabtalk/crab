//! `berm.vouch`: a service naming the end user the rest of its chain runs for.

use anyhow::Result;
use berm_api::Output;
use berm_host::test::{self, TestHost, User};

async fn deployed() -> Result<TestHost> {
    let host = TestHost::open(0).await?;
    host.deploy("fixture", &test::build("berm-fixture")?)
        .await?;
    Ok(host)
}

#[tokio::test]
async fn a_vouched_user_reaches_the_calls_made_after() -> Result<()> {
    let host = deployed().await?;
    let Output::Done(answer) = host
        .call("fixture", "vouched", br#"{"subject":"ada"}"#)
        .await?
        .result
    else {
        panic!("vouched answers");
    };
    let said: String = serde_json::from_str(&answer)?;
    assert_eq!(said, "service fixture user fixture ada");
    Ok(())
}

#[tokio::test]
async fn a_chain_that_runs_for_a_user_is_refused() -> Result<()> {
    let host = deployed().await?;
    let user = User {
        issuer: "https://issuer.example.com".into(),
        subject: "ada".into(),
    };
    let Output::Failed(failure) = host
        .call_for("fixture", "vouched", br#"{"subject":"grace"}"#, user)
        .await?
        .result
    else {
        panic!("vouching again is refused");
    };
    assert!(failure.contains("already runs for a user"), "{failure}");
    Ok(())
}

#[tokio::test]
async fn an_empty_subject_is_refused() -> Result<()> {
    let host = deployed().await?;
    let Output::Failed(failure) = host
        .call("fixture", "vouched", br#"{"subject":""}"#)
        .await?
        .result
    else {
        panic!("an empty subject is refused");
    };
    assert!(failure.contains("names someone"), "{failure}");
    Ok(())
}
