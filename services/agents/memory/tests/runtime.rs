use anyhow::Result;
use berm_api::Output;
use berm_build::fixture;
use berm_host::test::TestHost;
use serde_json::json;

#[tokio::test]
#[ignore = "requires a prepared Wasm image; see LIBRARY.md"]
async fn revisions_pagination_namespaces_and_restart_preserve_memories() -> Result<()> {
    let mut host = TestHost::open(10_000).await?;
    host.deploy("memory", &fixture::image("crab-agent-memory")?)
        .await?;
    let first = host.call_json("memory", "put", json!({"namespace":"project","key":"a","content":"Release notes","expected_revision":0})).await?;
    let revision = first["revision"].as_u64().unwrap();
    let conflict = host
        .call(
            "memory",
            "put",
            br#"{"namespace":"project","key":"a","content":"Overwrite","expected_revision":0}"#,
        )
        .await?;
    assert!(matches!(conflict.result, Output::Failed(_)));
    let updated = host.call_json("memory", "put", json!({"namespace":"project","key":"a","content":"Updated notes","expected_revision":revision})).await?;
    host.call_json(
        "memory",
        "put",
        json!({"namespace":"project","key":"b","content":"Other notes"}),
    )
    .await?;
    host.call_json(
        "memory",
        "put",
        json!({"namespace":"other","key":"c","content":"Hidden notes"}),
    )
    .await?;
    let page = host
        .call_json(
            "memory",
            "search",
            json!({"namespace":"project","query":"NOTES","limit":1}),
        )
        .await?;
    assert_eq!(page["items"][0]["key"], "a");
    assert_eq!(page["next"], "a");
    let next = host
        .call_json(
            "memory",
            "search",
            json!({"namespace":"project","query":"notes","after":page["next"],"limit":1}),
        )
        .await?;
    assert_eq!(next["items"].as_array().unwrap().len(), 1);
    assert_eq!(next["items"][0]["key"], "b");
    assert!(next.get("next").is_none());
    let conflict = host
        .call(
            "memory",
            "delete",
            &serde_json::to_vec(
                &json!({"namespace":"project","key":"a","expected_revision":revision}),
            )?,
        )
        .await?;
    assert!(matches!(conflict.result, Output::Failed(_)));
    host.reopen().await?;
    assert_eq!(
        host.call_json("memory", "get", json!({"namespace":"project","key":"a"}))
            .await?["content"],
        "Updated notes"
    );
    host.call_json(
        "memory",
        "delete",
        json!({"namespace":"project","key":"a","expected_revision":updated["revision"]}),
    )
    .await?;
    let recreated = host
        .call_json(
            "memory",
            "put",
            json!({"namespace":"project","key":"a","content":"Recreated","expected_revision":0}),
        )
        .await?;
    assert!(recreated["revision"].as_u64().unwrap() > updated["revision"].as_u64().unwrap());
    let conflict = host.call("memory", "put", &serde_json::to_vec(&json!({"namespace":"project","key":"a","content":"Stale writer","expected_revision":updated["revision"]}))?).await?;
    assert!(matches!(conflict.result, Output::Failed(_)));
    Ok(())
}

#[tokio::test]
#[ignore = "requires a prepared Wasm image; see LIBRARY.md"]
async fn maximum_escaped_content_fits_a_paginated_reply_and_oversize_fails() -> Result<()> {
    let host = TestHost::open(10_000).await?;
    host.deploy("memory", &fixture::image("crab-agent-memory")?)
        .await?;
    for key in 0..10 {
        host.call_json(
            "memory",
            "put",
            json!({"namespace":"large","key":key.to_string(),"content":"\u{0001}".repeat(16384)}),
        )
        .await?;
    }
    let found = host
        .call_json("memory", "search", json!({"namespace":"large","query":""}))
        .await?;
    assert_eq!(found["items"].as_array().unwrap().len(), 10);
    let refused = host
        .call(
            "memory",
            "put",
            &serde_json::to_vec(
                &json!({"namespace":"large","key":"too-big","content":"a".repeat(16385)}),
            )?,
        )
        .await?;
    assert!(matches!(refused.result, Output::Failed(_)));
    Ok(())
}
