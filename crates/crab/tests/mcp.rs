use anyhow::Result;
use axum::{
    Router,
    body::{self, Body},
    http::{HeaderMap, Request, StatusCode},
};
use berm_api::host::{self, Deploy};
use crab_agent::host::Node;
use serde_json::{Value, json};
use std::{fs, path::Path, time::Duration};
use tokio::{task, time};
use tower::ServiceExt;

struct Client {
    router: Router,
    session: Option<String>,
}

impl Client {
    async fn post(
        &self,
        message: Value,
        caller: Option<&str>,
    ) -> Result<(StatusCode, HeaderMap, Value)> {
        let mut request = Request::post("/mcp")
            .header("host", "localhost")
            .header("content-type", "application/json")
            .header("accept", "application/json, text/event-stream")
            .header("mcp-protocol-version", "2025-06-18");
        if let Some(session) = &self.session {
            request = request.header("mcp-session-id", session);
        }
        if let Some(caller) = caller {
            request = request.header(host::CALLER, caller);
        }
        let response = self
            .router
            .clone()
            .oneshot(request.body(Body::from(message.to_string()))?)
            .await?;
        let (parts, body) = response.into_parts();
        let bytes = time::timeout(Duration::from_secs(10), body::to_bytes(body, 1 << 20)).await??;
        let value = serde_json::from_slice(&bytes).unwrap_or_else(|_| {
            String::from_utf8_lossy(&bytes)
                .lines()
                .filter_map(|line| line.strip_prefix("data: "))
                .find_map(|data| serde_json::from_str(data).ok())
                .unwrap_or(Value::Null)
        });
        Ok((parts.status, parts.headers, value))
    }

    async fn call(&self, name: &str, arguments: Value, caller: Option<&str>) -> Result<Value> {
        let (status, _, message) = self
            .post(
                json!({
                    "jsonrpc": "2.0", "id": 2, "method": "tools/call",
                    "params": {"name": name, "arguments": arguments}
                }),
                caller,
            )
            .await?;
        assert_eq!(status, StatusCode::OK);
        assert!(message.get("error").is_none(), "{message}");
        Ok(message["result"].clone())
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn mcp_uses_each_requests_caller_for_discovery_and_execution() -> Result<()> {
    let image = task::spawn_blocking(|| -> Result<Vec<u8>> {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/fixture");
        Ok(fs::read(berm_build::build(&dir, None)?)?)
    })
    .await??;
    let root = tempfile::tempdir()?;
    let node = Node::new(
        root.path().into(),
        root.path().join("secrets.toml"),
        8,
        Duration::from_secs(1),
        Duration::from_secs(30),
        None,
    )
    .await?;
    let digest = node.keep(&image).await?;
    let deploy: Deploy =
        serde_json::from_value(json!({"reference": digest, "digest": digest, "force": true}))?;
    node.deploy("fixture", deploy).await?;
    let mut client = Client {
        router: node.router(),
        session: None,
    };
    let (status, headers, initialized) = client
        .post(
            json!({
                "jsonrpc": "2.0", "id": 1, "method": "initialize",
                "params": {"protocolVersion": "2025-06-18", "capabilities": {},
                    "clientInfo": {"name": "test", "version": "0"}}
            }),
            None,
        )
        .await?;
    assert_eq!(status, StatusCode::OK);
    assert!(initialized["result"].is_object(), "{initialized}");
    client.session = Some(headers["mcp-session-id"].to_str()?.to_owned());
    let (status, _, _) = client
        .post(
            json!({
                "jsonrpc": "2.0", "method": "notifications/initialized"
            }),
            None,
        )
        .await?;
    assert_eq!(status, StatusCode::ACCEPTED);

    let anonymous = client
        .call("search", json!({"query": "fixture"}), None)
        .await?;
    assert_eq!(anonymous["content"][0]["text"], "[]");
    let alice = r#"{"id":"alice"}"#;
    let found = client
        .call("search", json!({"query": "fixture"}), Some(alice))
        .await?;
    let found: Value = serde_json::from_str(found["content"][0]["text"].as_str().unwrap())?;
    let tools = found[0]["tools"].as_array().unwrap();
    assert!(tools.iter().any(|tool| tool["name"] == "whoami"));
    assert!(!tools.iter().any(|tool| tool["name"] == "fromservices"));

    assert!(!tools.iter().any(|tool| tool["name"] == "tick"));

    let input = json!({"service": "fixture", "tool": "whoami"});
    let called = client.call("call", input.clone(), Some(alice)).await?;
    assert_eq!(called["content"][0]["text"], r#""outside alice""#);
    let called = client
        .call("call", input.clone(), Some(r#"{"id":"bob"}"#))
        .await?;
    assert_eq!(called["content"][0]["text"], r#""outside bob""#);
    for caller in [None, Some(r#"{"id":"alice","app":"app"}"#)] {
        let called = client.call("call", input.clone(), caller).await?;
        assert_eq!(called["isError"], true);
        let found = client
            .call("search", json!({"query": "fixture"}), caller)
            .await?;
        assert_eq!(found["content"][0]["text"], "[]");
    }
    let called = client
        .call(
            "call",
            json!({"service": "fixture", "tool": "fromservices"}),
            Some(alice),
        )
        .await?;
    assert_eq!(called["isError"], true);
    let called = client
        .call(
            "call",
            json!({"service": "fixture", "tool": "tick"}),
            Some(alice),
        )
        .await?;
    assert_eq!(called["isError"], true);
    let called = client.call("call", input, Some(r#"{"id":"alice","app":"app","user":{"issuer":"https://id.example","subject":"42"}}"#)).await?;
    assert_eq!(
        called["content"][0]["text"],
        r#""outside alice user https://id.example 42""#
    );
    let (status, _, _) = client
        .post(
            json!({"jsonrpc": "2.0", "id": 3, "method": "tools/list"}),
            Some("invalid"),
        )
        .await?;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    Ok(())
}
