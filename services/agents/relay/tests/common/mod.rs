#![allow(dead_code)]
use anyhow::{Result, bail};
use berm_api::{Manifest, Output, host::Caller};
use berm_build::fixture;
use berm_host::test::{TestHost, image};
use serde_json::{Value, json};
use std::ops::{Deref, DerefMut};

pub struct Relay(TestHost);

impl Deref for Relay {
    type Target = TestHost;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for Relay {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl Relay {
    pub async fn open() -> Result<Self> {
        let host = TestHost::open(1000).await?;
        let bytes = fixture::image("crab-agent-relay")?;
        let mut deps = Manifest::try_from(bytes.as_slice())?.deps;
        deps.http.fetch = vec!["bots.example".into()];
        host.deploy("relay", &image::with_deps(&bytes, deps)?)
            .await?;
        Ok(Self(host))
    }

    pub async fn call(&self, tool: &str, input: Value) -> Result<Value> {
        self.call_as("alice", None, tool, input).await
    }

    pub async fn call_as(
        &self,
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
        let (_, output) = self
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

    pub async fn register(&self, owner: &str, name: &str) -> Result<u64> {
        Ok(self
            .call_as(
                owner,
                None,
                "register",
                json!({"name":name,"platform":{"open_ai_dot":{}}}),
            )
            .await?["id"]
            .as_u64()
            .unwrap())
    }

    pub async fn conversation(&self, members: &[u64]) -> Result<u64> {
        Ok(self.call("open", json!({"members":members})).await?["id"]
            .as_u64()
            .unwrap())
    }
}
