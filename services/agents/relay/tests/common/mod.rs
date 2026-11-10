#![allow(dead_code)]
use anyhow::{Result, bail};
use berm_api::{Image, Manifest, Output, host::Caller};
use berm_build::fixture;
use berm_host::test::TestHost;
use serde_json::{Value, json};

pub async fn call(host: &TestHost, owner: &str, tool: &str, input: Value) -> Result<Value> {
    call_as(host, owner, None, tool, input).await
}

pub async fn call_as(
    host: &TestHost,
    owner: &str,
    key: Option<&str>,
    tool: &str,
    input: Value,
) -> Result<Value> {
    let caller = Caller {
        id: owner.into(),
        app: None,
        user: None,
        client: None,
        agent: None,
        key: key.map(Into::into),
    };
    let (_, output) = host
        .host
        .call(
            "relay",
            Some(&caller),
            tool,
            serde_json::to_vec(&input)?,
            false,
        )
        .await?;
    match output {
        Output::Done(value) if value.is_empty() => Ok(Value::Null),
        Output::Done(value) => Ok(serde_json::from_str(&value)?),
        Output::Failed(error) => bail!("{error}"),
    }
}

pub async fn opened() -> Result<TestHost> {
    let host = TestHost::open(1000).await?;
    let image = fixture::image("crab-agent-relay")?;
    let mut deps = Manifest::try_from(image.as_slice())?.deps;
    deps.http.fetch = vec!["bots.example".into()];
    host.deploy("relay", &Image(&image).with_deps(deps)?)
        .await?;
    Ok(host)
}

pub async fn register(host: &TestHost, owner: &str, name: &str) -> Result<u64> {
    Ok(call(
        host,
        owner,
        "register",
        json!({"name":name,"platform":{"open_ai_dot":{}}}),
    )
    .await?["id"]
        .as_u64()
        .unwrap())
}
