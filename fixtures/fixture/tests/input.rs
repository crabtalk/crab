//! What a call ran with: kept for a call that failed, held in memory and once
//! written, and for no other.

use anyhow::Result;
use berm_api::host::{CallPage, Caller, Outcome, User};
use berm_host::test::{self, TestHost};

#[tokio::test]
async fn a_failed_call_keeps_its_input_and_a_done_one_none() -> Result<()> {
    let host = TestHost::open(0).await?;
    let image = test::build("berm-fixture")?;
    host.deploy("acct/fixture", &image).await?;
    let caller = Caller {
        id: "acct".to_owned(),
        app: None,
        user: Some(User {
            issuer: "https://id.example".to_owned(),
            subject: "42".to_owned(),
        }),
        client: None,
        agent: None,
    };
    let call = |tool: &'static str, args: &'static [u8]| {
        host.host()
            .call("acct/fixture", Some(&caller), tool, args.to_vec(), false)
    };
    call("echo", br#"{"query":"hi"}"#).await?;
    call("boom", br#"{"why":"test"}"#).await?;

    for written in [false, true] {
        if written {
            host.flush().await?;
        }
        let (calls, _) = host
            .host()
            .calls(
                "acct/fixture",
                CallPage {
                    service: None,
                    tool: None,
                    event: None,
                    q: None,
                    outcome: None,
                    since: None,
                    until: None,
                    before: None,
                    limit: 10,
                },
            )
            .await?;
        let failed = calls.iter().find(|c| c.outcome == Outcome::Failed).unwrap();
        let done = calls.iter().find(|c| c.outcome == Outcome::Done).unwrap();

        let input = host.host().call_input("acct/fixture", failed.id).await?;
        assert_eq!(input.tool, "boom");
        assert_eq!(input.args, r#"{"why":"test"}"#);
        let kept = input.caller.unwrap();
        assert_eq!(kept.id, "acct");
        assert_eq!(kept.user.unwrap().subject, "42");
        assert_eq!(
            input.digest,
            format!("sha256:{}", host.host().get("acct/fixture").unwrap().digest)
        );

        let error = host
            .host()
            .call_input("acct/fixture", done.id)
            .await
            .unwrap_err();
        assert!(format!("{error:?}").contains("kept no input"), "{error:?}");
        assert!(host.host().call_input("acct/fixture", 999).await.is_err());
    }
    Ok(())
}
