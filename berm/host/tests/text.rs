use anyhow::Result;
use berm_api::Output;
use berm_build::fixture;
use berm_host::test::TestHost;
use serde_json::json;

#[tokio::test]
async fn text_uses_utf8_bytes_and_rejects_invalid_existing_values() -> Result<()> {
    let host = TestHost::open(0).await?;
    host.deploy("text", &fixture::image("berm-fixture")?)
        .await?;
    let empty = host
        .call_json("text", "stored_text", json!({"key":"missing"}))
        .await?;
    assert_eq!(empty, json!({}));
    let text = "Hello, 世界\n\"quoted\"\\slash";
    let stored = host
        .call_json("text", "stored_text", json!({"key":"kept","text":text}))
        .await?;
    assert_eq!(stored, json!({"text":text,"bytes":text.as_bytes()}));
    assert_eq!(
        host.call_json("text", "stored_text", json!({"key":"kept"}))
            .await?,
        stored
    );
    assert_eq!(
        host.call_json("text", "stored_text", json!({"key":"kept","text":""}))
            .await?,
        json!({"text":"","bytes":[]})
    );
    let denied = host
        .call("text", "stored_text", br#"{"key":"invalid","bytes":[255]}"#)
        .await?;
    assert_eq!(
        denied.result,
        Output::Failed("invalid holds a value that isn't UTF-8".into())
    );
    Ok(())
}
