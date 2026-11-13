use anyhow::Result;
use berm_api::{Output, host::Caller};
use berm_build::fixture;
use berm_host::test::TestHost;
use serde_json::{Value, json};

#[tokio::test]
#[ignore = "needs BERM_TEST_IMAGE_CRAB_AGENT_OUTBOX"]
async fn requests_send_only_as_the_owner_and_retries_keep_the_provider_key() -> Result<()> {
    let host = TestHost::open(1000).await?;
    host.deploy("outbox", &fixture::image("crab-agent-outbox")?)
        .await?;
    host.grant("outbox", "RESEND_API_KEY", "api.resend.com", "provider-key");
    host.arrange(
        "POST",
        "https://api.resend.com/emails",
        503,
        r#"{"message":"Try later"}"#,
    );
    let mut caller = Caller {
        id: "alice".into(),
        key: Some("assistant-key".into()),
        app: None,
        user: None,
        client: None,
        agent: None,
    };
    let input = json!({"from":"hello@example.com","to":["reader@example.com"],"subject":"Report","text":"Ready"});
    let (_, review) = host
        .host
        .call(
            "outbox",
            Some(&caller),
            "review",
            serde_json::to_vec(&json!({"input":input}))?,
            false,
        )
        .await?;
    let Output::Done(review) = review else {
        panic!("review failed")
    };
    let review: Value = serde_json::from_str(&review)?;
    assert_eq!(review["input"], input);
    assert!(host.fetched().is_empty());
    let args = serde_json::to_vec(&json!({"request":"BCDF-GHJK","input":input}))?;
    let (_, denied) = host
        .host
        .call("outbox", Some(&caller), "send", args.clone(), false)
        .await?;
    assert!(matches!(denied, Output::Failed(_)));
    assert!(host.fetched().is_empty());
    caller.key = None;
    let (_, failed) = host
        .host
        .call("outbox", Some(&caller), "send", args.clone(), false)
        .await?;
    assert!(matches!(failed, Output::Failed(_)));
    host.arrange(
        "POST",
        "https://api.resend.com/emails",
        200,
        r#"{"id":"mail-1"}"#,
    );
    let (_, done) = host
        .host
        .call("outbox", Some(&caller), "send", args, false)
        .await?;
    assert!(matches!(done, Output::Done(_)));
    let fetched = host.fetched();
    assert_eq!(fetched.len(), 2);
    for request in &fetched {
        assert!(
            request
                .headers
                .iter()
                .any(|(name, value)| name.eq_ignore_ascii_case("idempotency-key")
                    && value == "outbox/alice/BCDF-GHJK")
        );
        assert_eq!(serde_json::from_slice::<Value>(&request.body)?, input);
    }
    let idl = host.host.idl("outbox")?;
    let builder = idl.tools.iter().find(|tool| tool.name == "review").unwrap();
    assert!(builder.request && builder.query);
    let refused = host
        .host
        .outside(
            "outbox",
            "send",
            false,
            serde_json::to_vec(&json!({"request":"BCDF-GHJK","input":input}))?,
            Some(caller),
            [("if-match".parse()?, "\"sha256:stale\"".parse()?)]
                .into_iter()
                .collect(),
        )
        .await
        .unwrap_err();
    assert_eq!(refused.status.as_u16(), 412);
    assert_eq!(host.fetched().len(), 2);
    Ok(())
}

#[tokio::test]
#[ignore = "needs BERM_TEST_IMAGE_CRAB_AGENT_OUTBOX"]
async fn maximum_draft_fits_the_action_budget() -> Result<()> {
    let host = TestHost::open(1000).await?;
    host.deploy("outbox", &fixture::image("crab-agent-outbox")?)
        .await?;
    host.grant("outbox", "RESEND_API_KEY", "api.resend.com", "provider-key");
    host.arrange(
        "POST",
        "https://api.resend.com/emails",
        200,
        r#"{"id":"mail-1"}"#,
    );
    let draft = json!({"from":format!("{}@example.com","a".repeat(242)),"to":vec![format!("{}@example.com","b".repeat(242));5],"subject":"S".repeat(200),"text":format!("{}{}",format!("{}\n","x".repeat(39)).repeat(99),"x".repeat(40))});
    let ran = host
        .host
        .execute(
            "outbox",
            "send",
            serde_json::to_vec(&json!({"request":"BCDF-GHJK","input":draft}))?,
            Some("alice"),
            None,
            Default::default(),
        )
        .await?;
    assert!(matches!(ran.result, Output::Done(_)), "{:?}", ran.result);
    assert_eq!(host.fetched().len(), 1);
    Ok(())
}
