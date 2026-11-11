use anyhow::Result;
use berm_api::{
    Output,
    host::{Caller, Link},
};
use berm_build::fixture;
use berm_host::test::TestHost;
use serde_json::json;

#[tokio::test]
async fn calls_use_deployment_addresses_without_an_allowlist() -> Result<()> {
    let host = TestHost::open(0).await?;
    let image = fixture::image("berm-fixture")?;
    host.deploy("alice/a", &image).await?;
    host.deploy("alice/b", &image).await?;
    for target in ["b", "alice/b"] {
        let args = json!({"service":target,"tool":"whoami","args":"{}"}).to_string();
        let ran = host.call("alice/a", "invoke", args.as_bytes()).await?;
        assert_eq!(
            ran.result,
            Output::Done(serde_json::to_string(&serde_json::to_string(
                "service alice/a"
            )?)?)
        );
    }
    host.deploy("bob/b", &image).await?;
    let args = json!({"service":"bob/b","tool":"whoami","args":"{}"}).to_string();
    let ran = host.call("alice/a", "invoke", args.as_bytes()).await?;
    assert!(matches!(ran.result, Output::Failed(message) if message.contains("account gateway")));
    Ok(())
}

#[tokio::test]
async fn authenticated_key_ids_survive_nested_calls_and_are_absent_for_sessions() -> Result<()> {
    let host = TestHost::open(0).await?;
    let image = fixture::image("berm-fixture")?;
    host.deploy("alice/a", &image).await?;
    host.deploy("alice/b", &image).await?;
    let caller = Caller {
        id: "alice".into(),
        key: Some("research-key".into()),
        app: None,
        user: None,
        client: None,
        agent: None,
    };
    let (_, direct) = host
        .host
        .call("alice/a", Some(&caller), "whoami", b"{}".to_vec(), false)
        .await?;
    assert_eq!(
        direct,
        Output::Done(serde_json::to_string("outside alice key research-key")?)
    );
    let args = json!({"service":"b","tool":"whoami","args":"{}"});
    let (_, nested) = host
        .host
        .call(
            "alice/a",
            Some(&caller),
            "invoke",
            serde_json::to_vec(&args)?,
            false,
        )
        .await?;
    assert_eq!(
        nested,
        Output::Done(serde_json::to_string(&serde_json::to_string(
            "service alice/a key research-key"
        )?)?)
    );
    let link = Link {
        caller: Some("alice/a".into()),
        from: "alice/a".into(),
        key: caller.key.clone(),
        user: None,
        client: None,
        client_ip: None,
        depth: 2,
        query: false,
        timeout: 5000,
        stack: vec!["alice/a".into(), "alice/b".into()],
    };
    let transported = serde_json::from_slice(&serde_json::to_vec(&link)?)?;
    let linked = host
        .host
        .link("alice/b", "whoami", b"{}".to_vec(), transported)
        .await?;
    assert_eq!(
        linked.result,
        Output::Done(serde_json::to_string("service alice/a key research-key")?)
    );
    let session = Caller {
        key: None,
        ..caller
    };
    let (_, output) = host
        .host
        .call("alice/a", Some(&session), "whoami", b"{}".to_vec(), false)
        .await?;
    assert_eq!(
        output,
        Output::Done(serde_json::to_string("outside alice")?)
    );
    Ok(())
}
