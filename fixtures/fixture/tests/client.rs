use anyhow::Result;
use berm::{
    Berm, Calls, Client, Config, Engine, chain,
    tape::{Recorder, Taping},
};
use berm_api::{
    Output,
    host::{Caller, Link},
};
use berm_build::fixture;
use berm_host::test::TestHost;
use serde_json::json;
use std::sync::Arc;

#[tokio::test]
async fn client_key_and_ip_reach_nested_calls() -> Result<()> {
    let host = TestHost::open(0).await?;
    let image = fixture::image("berm-fixture")?;
    for name in ["acct/fixture", "acct/inner"] {
        host.deploy(name, &image).await?;
    }
    for ip in ["203.0.113.42", "2001:db8::42"] {
        for args in [b"{}".as_slice(), br#"{"target":"inner"}"#] {
            let ran = host
                .call_with_client(
                    "acct/fixture",
                    "client_context",
                    args,
                    Client {
                        key: Some("daily-key".into()),
                        ip: Some(ip.parse()?),
                    },
                )
                .await?;
            assert_eq!(
                ran.result,
                Output::Done(json!(["daily-key", ip]).to_string())
            );
        }
    }
    assert_eq!(
        host.call_json("acct/fixture", "client_context", json!({}))
            .await?,
        json!([null, null])
    );
    Ok(())
}

#[tokio::test]
async fn peer_links_preserve_both_client_values() -> Result<()> {
    let host = TestHost::open(0).await?;
    host.deploy("acct/fixture", &fixture::image("berm-fixture")?)
        .await?;
    let link: Link = serde_json::from_value(json!({
        "caller":"sender", "from":"acct/sender", "client":"daily-key",
        "client_ip":"2001:db8::42", "depth":2, "query":false,
        "timeout":5000, "stack":["acct/sender", "acct/fixture"]
    }))?;
    let reply = host
        .host
        .link("acct/fixture", "client_context", b"{}".to_vec(), link)
        .await?;
    assert_eq!(
        reply.result,
        Output::Done(json!(["daily-key", "2001:db8::42"]).to_string())
    );
    Ok(())
}

#[tokio::test]
async fn client_context_survives_tape_replay() -> Result<()> {
    let engine = Engine::new(&Config::default())?;
    let runtime = Berm::new(&engine, Calls::by_name(0), Vec::new());
    let service = runtime.deploy("fixture", &fixture::image("berm-fixture")?)?;
    let client = Client {
        key: Some("daily-key".into()),
        ip: Some("203.0.113.42".parse()?),
    };
    let recorder = Recorder::default();
    let recorded = service
        .call_as(
            None,
            None,
            None,
            client.clone(),
            "client_context",
            b"{}".to_vec(),
            Arc::default(),
            chain::next(),
            Taping::Record(recorder.clone()),
        )
        .await?;
    let replayed = service
        .call_as(
            None,
            None,
            None,
            client,
            "client_context",
            b"{}".to_vec(),
            Arc::default(),
            chain::next(),
            Taping::Replay {
                tape: recorder.tape(),
                through: Vec::new(),
            },
        )
        .await?;
    assert_eq!(recorded.result, replayed.result);
    assert_eq!(
        replayed.result,
        Output::Done(json!(["daily-key", "203.0.113.42"]).to_string())
    );
    Ok(())
}

#[tokio::test]
async fn outside_calls_use_the_host_supplied_address() -> Result<()> {
    let host = TestHost::open(0).await?;
    host.deploy("acct/fixture", &fixture::image("berm-fixture")?)
        .await?;
    let caller = Caller {
        id: "acct".to_owned(),
        app: None,
        user: None,
        client: Some("2001:db8::42".to_owned()),
        agent: None,
        key: None,
    };
    let (_, result) = host
        .host
        .call(
            "acct/fixture",
            Some(&caller),
            "client_context",
            b"{}".to_vec(),
            false,
        )
        .await?;
    assert_eq!(
        result,
        Output::Done(json!([null, "2001:db8::42"]).to_string())
    );
    Ok(())
}
