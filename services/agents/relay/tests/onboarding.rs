use crate::common::Relay;
use anyhow::Result;
use serde_json::json;

mod common;

#[tokio::test]
async fn approved_named_agents_exchange_messages_without_manual_conversations() -> Result<()> {
    let host = Relay::open().await?;
    let other = host
        .call_as(
            "alice",
            Some("research"),
            "register",
            json!({"handle":"research-bot"}),
        )
        .await?["handle"]
        .as_str()
        .unwrap()
        .to_owned();
    let input = json!({"handle":"assistant","name":"My assistant"});
    let connected = host
        .call_as("alice", Some("chatgpt"), "register", input.clone())
        .await?;
    assert_eq!(
        host.call_as("alice", Some("chatgpt"), "register", input)
            .await?,
        connected
    );
    let assistant = connected["handle"].as_str().unwrap();
    let contacts = host
        .call_as("alice", Some("chatgpt"), "contacts", json!({}))
        .await?;
    assert_eq!(contacts["items"].as_array().unwrap().len(), 2);
    let initial = host
        .call_as("alice", Some("chatgpt"), "receive_events", json!({}))
        .await?;
    let request = json!({"to":"@RESEARCH-BOT","body":"Suggest three names","key":"demo"});
    let message = host
        .call_as("alice", Some("chatgpt"), "message", request.clone())
        .await?;
    assert_eq!(
        host.call_as("alice", Some("chatgpt"), "message", request)
            .await?,
        message
    );
    assert_eq!(message["from"], assistant);
    assert_eq!(message["to"], other);
    let reply = host
        .call_as(
            "alice",
            Some("research"),
            "reply",
            json!({"message":message["id"],"body":"Relay, Dispatch, or Roundtable","key":"reply"}),
        )
        .await?;
    assert_eq!(reply["conversation"], message["conversation"]);
    assert_eq!(reply["to"], assistant);
    assert_eq!(reply["reply_to"], message["id"]);
    host.call(
        "connect",
        json!({"key_id":"chatgpt","agents":["assistant","research-bot"]}),
    )
    .await?;
    host.call(
        "update_agent",
        json!({"handle":assistant,"name":"Renamed assistant"}),
    )
    .await?;
    let events = host
        .call_as(
            "alice",
            Some("chatgpt"),
            "receive_events",
            json!({"cursor":initial["cursor"]}),
        )
        .await?;
    assert_eq!(events["events"][0]["data"], reply);
    let next = host
        .call_as(
            "alice",
            Some("chatgpt"),
            "message",
            json!({"from":"assistant","to":"research-bot","body":"Choose your favorite","key":"followup"}),
        )
        .await?;
    assert_eq!(next["conversation"], message["conversation"]);
    Ok(())
}

#[tokio::test]
async fn connection_access_requires_owner_approval_and_revocation_stops_events() -> Result<()> {
    let host = Relay::open().await?;
    host.register("alice", "Assistant").await?;
    host.register("bob", "Assistant").await?;
    let input = json!({"key_id":"chatgpt","agents":["assistant"]});
    assert!(
        host.call_as("alice", Some("chatgpt"), "connect", input.clone())
            .await
            .is_err()
    );
    assert!(
        host.call_as(
            "alice",
            Some("chatgpt"),
            "receive_events",
            json!({"agent":"assistant"})
        )
        .await
        .is_err()
    );
    host.call("connect", input).await?;
    host.call_as("alice", Some("chatgpt"), "receive_events", json!({}))
        .await?;
    assert!(
        host.call_as(
            "bob",
            Some("chatgpt"),
            "receive_events",
            json!({"agent":"assistant"})
        )
        .await
        .is_err()
    );
    assert!(
        host.call("connect", json!({"key_id":"chatgpt","agents":["Missing"]}))
            .await
            .is_err()
    );
    assert_eq!(
        host.call("access", json!({"key_id":"chatgpt"})).await?["items"][0]["granted"],
        true
    );
    host.call("connect", json!({"key_id":"chatgpt","agents":[]}))
        .await?;
    assert!(
        host.call_as(
            "alice",
            Some("chatgpt"),
            "receive_events",
            json!({"agent":"assistant"})
        )
        .await
        .is_err()
    );
    Ok(())
}

#[tokio::test]
async fn shared_connections_select_named_senders_and_preserve_pause_and_budget() -> Result<()> {
    let host = Relay::open().await?;
    let first = host.register("alice", "First").await?;
    let second = host.register("alice", "Second").await?;
    host.register("alice", "Recipient").await?;
    assert!(
        host.call_as(
            "alice",
            Some("other-key"),
            "register",
            json!({"handle":"first"})
        )
        .await
        .is_err()
    );
    host.call(
        "connect",
        json!({"key_id":"shared","agents":["first","second"]}),
    )
    .await?;
    assert!(
        host.call_as(
            "alice",
            Some("shared"),
            "message",
            json!({"to":"Recipient","body":"Ambiguous","key":"ambiguous"})
        )
        .await
        .is_err()
    );
    let conversation = host
        .call("open", json!({"members":[first,second],"budget":1}))
        .await?;
    let message = host
        .call_as(
            "alice",
            Some("shared"),
            "message",
            json!({"from":"First","to":"Second","body":"One","key":"one"}),
        )
        .await?;
    assert_eq!(message["conversation"], conversation["id"]);
    assert!(
        host.call_as(
            "alice",
            Some("shared"),
            "message",
            json!({"from":"First","to":"Second","body":"Two","key":"two"})
        )
        .await
        .is_err()
    );
    host.call(
        "pause",
        json!({"conversation":conversation["id"],"paused":true}),
    )
    .await?;
    assert!(
        host.call_as(
            "alice",
            Some("shared"),
            "message",
            json!({"from":"First","to":"Second","body":"Paused","key":"paused"})
        )
        .await
        .is_err()
    );
    assert_eq!(
        host.call("read", json!({"conversation":conversation["id"]}))
            .await?["items"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    Ok(())
}

#[tokio::test]
async fn paged_contacts_and_maximum_connection_and_message_inputs_fit_their_budgets() -> Result<()>
{
    let host = Relay::open().await?;
    let mut names = Vec::new();
    for index in 0..200 {
        let name = format!("agent-{index:03}{}", "x".repeat(55));
        host.register("alice", &name).await?;
        names.push(name);
    }
    let first = host.call("contacts", json!({})).await?;
    assert_eq!(first["items"].as_array().unwrap().len(), 100);
    let second = host
        .call("contacts", json!({"after":first["next"]}))
        .await?;
    assert_eq!(second["items"].as_array().unwrap().len(), 100);
    assert!(second["next"].is_null());
    let body = "\0".repeat(4096);
    let message = host
        .call(
            "message",
            json!({"from":names[0],"to":names[129],"body":body,"key":"seed"}),
        )
        .await?;
    let caller = berm_api::host::Caller {
        id: "alice".into(),
        key: None,
        app: None,
        user: None,
        client: None,
        agent: None,
    };
    for (tool, args) in [
        (
            "connect",
            json!({"key_id":"connection","agents":&names[..100]}),
        ),
        ("contacts", json!({})),
        ("access", json!({"key_id":"connection"})),
        (
            "message",
            json!({"from":names[0],"to":names[129],"body":body}),
        ),
        ("reply", json!({"message":message["id"],"body":body})),
        ("messages", json!({"agent":names[0]})),
        (
            "receive_events",
            json!({"cursor":format!("{}:0",message["from"].as_str().unwrap())}),
        ),
    ] {
        for run in 0..10 {
            let mut args = args.clone();
            if tool == "connect" {
                args["agents"] = json!(&names[if run % 2 == 0 { 0..100 } else { 100..200 }]);
            }
            if tool == "message" || tool == "reply" {
                args["key"] = json!(format!("{tool}-{run}"));
            }
            let (headers, result) = host
                .host
                .call(
                    "relay",
                    Some(&caller),
                    tool,
                    serde_json::to_vec(&args)?,
                    false,
                )
                .await?;
            assert!(
                matches!(result, berm_api::Output::Done(_)),
                "{tool}: {result:?}"
            );
            let meter = headers.get(berm_api::host::METER).unwrap().to_str()?;
            println!("{tool}: {meter}");
        }
    }
    Ok(())
}

#[tokio::test]
async fn self_registered_handles_are_stable_and_cannot_be_taken_over() -> Result<()> {
    let host = Relay::open().await?;
    let profile = host
        .call_as(
            "alice",
            Some("chatgpt"),
            "register",
            json!({"handle":"@My-Assistant","name":"Assistant"}),
        )
        .await?;
    assert_eq!(profile["handle"], "my-assistant");
    assert!(profile.get("id").is_none());
    assert_eq!(
        host.call_as(
            "alice",
            Some("chatgpt"),
            "register",
            json!({"handle":"my-assistant"})
        )
        .await?,
        profile
    );
    assert!(
        host.call_as(
            "alice",
            Some("other"),
            "register",
            json!({"handle":"MY-ASSISTANT"})
        )
        .await
        .is_err()
    );
    let other = host
        .call_as(
            "bob",
            Some("other"),
            "register",
            json!({"handle":"my-assistant","name":"Assistant"}),
        )
        .await?;
    assert_eq!(other["handle"], profile["handle"]);
    for handle in ["", "123", "has space", "bad/handle", "@@assistant"] {
        assert!(
            host.call_as(
                "alice",
                Some("chatgpt"),
                "register",
                json!({"handle":handle})
            )
            .await
            .is_err()
        );
    }
    host.call(
        "update_agent",
        json!({"handle":"my-assistant","name":"New display name"}),
    )
    .await?;
    assert_eq!(
        host.call_as("alice", Some("chatgpt"), "agents", json!({}))
            .await?[0]["handle"],
        "my-assistant"
    );
    host.call(
        "revoke_agent",
        json!({"agent":"my-assistant","key_id":"chatgpt"}),
    )
    .await?;
    assert!(
        host.call_as(
            "alice",
            Some("chatgpt"),
            "register",
            json!({"handle":"my-assistant"})
        )
        .await
        .is_err()
    );
    assert!(
        host.call_as(
            "alice",
            Some("chatgpt"),
            "messages",
            json!({"agent":"my-assistant"})
        )
        .await
        .is_err()
    );
    let idl = host.host.idl("relay")?;
    for tool in &idl.tools {
        if let Some(properties) = tool
            .parameters
            .get("properties")
            .and_then(serde_json::Value::as_object)
        {
            for field in ["agent", "from", "to", "handle"] {
                if let Some(schema) = properties.get(field) {
                    assert!(
                        !schema.to_string().contains("integer"),
                        "{}.{field}",
                        tool.name
                    );
                }
            }
        }
    }
    Ok(())
}
