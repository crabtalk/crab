use anyhow::Result;
use berm_api::{Output, host::Caller, workflow::run::Failure};
use berm_build::fixture;
use berm_host::test::TestHost;
use serde_json::json;

#[tokio::test]
async fn secret_resources_use_service_and_host_grants_without_declaring_each_id() -> Result<()> {
    let host = TestHost::open(0).await?;
    let image = fixture::image("berm-fixture")?;
    host.deploy("publisher/relay", &image).await?;
    host.deploy("publisher/other", &image).await?;
    let id = "secret_1111111111111111111111111111111111111111111111111111111111111111";
    let url = "http://127.0.0.1/routine";
    let input = json!({"url":url,"header":format!("Authorization: Bearer {{secret:{id}}}")});
    host.arrange("GET", url, 200, "ok");
    assert!(matches!(
        host.call("publisher/relay", "fetch", input.to_string().as_bytes())
            .await?
            .result,
        Output::Failed(_)
    ));
    assert!(host.fetched().is_empty());
    host.grant_resource("publisher/relay", id, "127.0.0.1", "publisher", "first-key");
    host.call_json("publisher/relay", "fetch", input.clone())
        .await?;
    assert_eq!(
        host.fetched()[0].headers,
        [("Authorization".into(), "Bearer first-key".into())]
    );
    host.grant("publisher/relay", id, "127.0.0.1", "rotated-key");
    host.call_json("publisher/relay", "fetch", input.clone())
        .await?;
    assert_eq!(
        host.fetched()[1].headers,
        [("Authorization".into(), "Bearer rotated-key".into())]
    );
    assert!(matches!(
        host.call("publisher/other", "fetch", input.to_string().as_bytes())
            .await?
            .result,
        Output::Failed(_)
    ));
    let missing = input
        .to_string()
        .replace(id, "secret_22222222222222222222222222222222");
    assert!(matches!(
        host.call("publisher/relay", "fetch", missing.as_bytes())
            .await?
            .result,
        Output::Failed(_)
    ));
    assert_eq!(host.fetched().len(), 2);
    Ok(())
}

#[tokio::test]
async fn cross_account_secret_requires_its_account_or_an_actual_wake() -> Result<()> {
    let host = TestHost::open(0).await?;
    host.deploy("publisher/relay", &fixture::image("berm-fixture")?)
        .await?;
    let id = "secret_11111111111111111111111111111111";
    let url = "http://127.0.0.1/routine";
    let input = json!({"url":url,"header":format!("Authorization: Bearer {{secret:{id}}}")});
    let args = serde_json::to_vec(&input)?;
    let mut caller: Caller = serde_json::from_value(json!({"id":"member", "account":"account-b"}))?;
    host.arrange("GET", url, 200, "ok");
    host.grant_resource("publisher/relay", id, "127.0.0.1", "account-a", "key-a");

    let error = host
        .call("publisher/relay", "fetch", &args)
        .await
        .err()
        .expect("an unscheduled invocation without a caller must be refused");
    let failure = error.downcast_ref::<Failure>().unwrap();
    assert_eq!(failure.status, 403);
    assert_eq!(failure.body["explain"], "E0089");

    let refused = host
        .host
        .call(
            "publisher/relay",
            Some(&caller),
            "fetch",
            args.clone(),
            false,
        )
        .await
        .unwrap_err();
    assert_eq!(refused.status.as_u16(), 403);
    assert_eq!(refused.failure.unwrap().body["explain"], "E0089");
    assert!(host.fetched().is_empty());

    caller.account = Some("account-a".into());
    let (_, output) = host
        .host
        .call("publisher/relay", Some(&caller), "fetch", args, false)
        .await?;
    assert!(matches!(output, Output::Done(_)));

    host.call_json(
        "publisher/relay",
        "invoke",
        json!({"service":"","tool":"fetch","args":input.to_string(),"delay":0}),
    )
    .await?;
    let wakes = host.fire().await?;
    assert_eq!(wakes.len(), 1);
    assert!(matches!(wakes[0].1.result, Output::Done(_)));
    assert_eq!(host.fetched().len(), 2);
    for request in host.fetched() {
        assert_eq!(
            request.headers,
            [("Authorization".into(), "Bearer key-a".into())]
        );
    }
    Ok(())
}

#[tokio::test]
async fn a_service_can_use_its_own_runtime_secret_for_another_account() -> Result<()> {
    let host = TestHost::open(0).await?;
    host.deploy("publisher/relay", &fixture::image("berm-fixture")?)
        .await?;
    let id = "secret_11111111111111111111111111111111";
    let url = "http://127.0.0.1/routine";
    let caller: Caller = serde_json::from_value(json!({"id":"member","account":"customer"}))?;
    let input = json!({"url":url,"header":format!("Authorization: Bearer {{secret:{id}}}")});
    host.arrange("GET", url, 200, "ok");
    host.grant_resource("publisher/relay", id, "127.0.0.1", "publisher", "key");
    let (_, output) = host
        .host
        .call(
            "publisher/relay",
            Some(&caller),
            "fetch",
            serde_json::to_vec(&input)?,
            false,
        )
        .await?;
    assert!(matches!(output, Output::Done(_)));
    assert_eq!(
        host.fetched()[0].headers,
        [("Authorization".into(), "Bearer key".into())]
    );
    Ok(())
}
