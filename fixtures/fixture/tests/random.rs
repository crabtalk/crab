//! `berm.random`, which berm serves itself with nothing to declare.

use anyhow::Result;
use berm_api::Output;
use berm_host::test::{self, TestHost};

#[tokio::test]
async fn each_call_answers_fresh_bytes() -> Result<()> {
    let host = TestHost::open(0).await?;
    host.deploy("fixture", &test::build("berm-fixture")?)
        .await?;
    let mut answered = Vec::new();
    for _ in 0..2 {
        let Output::Done(answer) = host.call("fixture", "random", b"{}").await?.result else {
            panic!("random answers");
        };
        let hex: String = serde_json::from_str(&answer)?;
        assert_eq!(hex.len(), 64, "{hex}");
        answered.push(hex);
    }
    assert_ne!(answered[0], answered[1]);
    Ok(())
}
