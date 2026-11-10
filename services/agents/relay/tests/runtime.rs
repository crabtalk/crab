use crate::common::{call, call_as, opened, register};
use anyhow::Result;
use berm_api::{events::Source, host::Caller};
use serde_json::json;

mod common;

#[tokio::test]
async fn several_bots_and_a_dot_keep_their_identities_and_history_after_restart() -> Result<()> {
    let mut host = opened().await?;
    host.grant_resource(
        "relay",
        "secret_11111111111111111111111111111111",
        "bots.example",
        "alice",
        "research-key",
    );
    host.grant_resource(
        "relay",
        "secret_22222222222222222222222222222222",
        "bots.example",
        "alice",
        "sales-key",
    );
    let research = call(&host, "alice", "register", json!({"name":"Research","platform":{"grok":{"webhook_url":"https://bots.example/research","secret_id":"secret_11111111111111111111111111111111"}}})).await?;
    let sales = call(&host, "alice", "register", json!({"name":"Sales","platform":{"grok":{"webhook_url":"https://bots.example/sales","secret_id":"secret_22222222222222222222222222222222"}}})).await?;
    let dot = register(&host, "alice", "Assistant").await?;
    let from = research["id"].as_u64().unwrap();
    assert_ne!(research["id"], sales["id"]);
    let conversation = call(&host, "alice", "open", json!({"members":[from,dot]})).await?["id"]
        .as_u64()
        .unwrap();
    let first = call(&host, "alice", "send", json!({"conversation":conversation,"from":from,"to":dot,"body":"Investigate this report","key":"request"})).await?;
    let reply = call(&host, "alice", "send", json!({"conversation":conversation,"from":dot,"to":from,"body":"Which customer?","key":"clarify","reply_to":first["id"]})).await?;
    let changed = call(
        &host,
        "alice",
        "update_agent",
        json!({"id":from,"name":"Researcher"}),
    )
    .await?;
    assert_eq!(changed["id"], from);
    assert_eq!(changed["platform"], research["platform"]);
    host.reopen().await?;
    let inbox = call(&host, "alice", "inbox", json!({"agent":from})).await?;
    assert_eq!(inbox["items"], json!([reply]));
    assert!(
        call(&host, "alice", "inbox", json!({"agent":sales["id"]})).await?["items"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    let agents = call(&host, "alice", "agents", json!({})).await?;
    assert_eq!(agents.as_array().unwrap().len(), 3);
    assert_eq!(agents[0]["name"], "Researcher");
    Ok(())
}

#[tokio::test]
async fn account_permissions_apply_to_registration_membership_and_message_history() -> Result<()> {
    let host = opened().await?;
    let alice = register(&host, "alice", "A").await?;
    let other = register(&host, "alice", "B").await?;
    let bob = register(&host, "bob", "C").await?;
    assert!(
        call(&host, "alice", "open", json!({"members":[alice,bob]}))
            .await
            .is_err()
    );
    assert!(
        call(
            &host,
            "bob",
            "update_agent",
            json!({"id":alice,"name":"Taken"})
        )
        .await
        .is_err()
    );
    let conversation = call(&host, "alice", "open", json!({"members":[alice,other]})).await?["id"]
        .as_u64()
        .unwrap();
    let input = json!({"conversation":conversation,"from":alice,"to":other,"body":"Private","key":"private"});
    call(&host, "alice", "send", input.clone()).await?;
    assert!(call(&host, "bob", "send", input).await.is_err());
    assert!(
        call(&host, "bob", "read", json!({"conversation":conversation}))
            .await
            .is_err()
    );
    assert!(
        call(&host, "bob", "inbox", json!({"agent":other}))
            .await
            .is_err()
    );
    assert_eq!(
        call(&host, "bob", "agents", json!({}))
            .await?
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert!(
        host.host
            .call(
                "relay",
                None,
                "register",
                serde_json::to_vec(&json!({"name":"Anonymous","platform":{"open_ai_dot":{}}}))?,
                false
            )
            .await
            .is_err()
    );
    Ok(())
}

#[tokio::test]
async fn retries_do_not_consume_budget_and_failures_leave_no_message() -> Result<()> {
    let host = opened().await?;
    let a = register(&host, "alice", "A").await?;
    let b = register(&host, "alice", "B").await?;
    let c = register(&host, "alice", "C").await?;
    let conversation =
        call(&host, "alice", "open", json!({"members":[a,b],"budget":1})).await?["id"]
            .as_u64()
            .unwrap();
    let input = json!({"conversation":conversation,"from":a,"to":b,"body":"Once","key":"once"});
    let mut invalid = input.clone();
    invalid["to"] = json!(c);
    assert!(call(&host, "alice", "send", invalid).await.is_err());
    let first = call(&host, "alice", "send", input.clone()).await?;
    assert_eq!(call(&host, "alice", "send", input.clone()).await?, first);
    let mut conflict = input.clone();
    conflict["body"] = json!("Changed");
    assert!(call(&host, "alice", "send", conflict).await.is_err());
    let mut second = input.clone();
    second["key"] = json!("twice");
    assert!(call(&host, "alice", "send", second).await.is_err());
    let paused = call(
        &host,
        "alice",
        "pause",
        json!({"conversation":conversation,"paused":true}),
    )
    .await?;
    assert_eq!(paused["sent"], 1);
    call(
        &host,
        "alice",
        "update_agent",
        json!({"id":a,"enabled":false}),
    )
    .await?;
    assert_eq!(call(&host, "alice", "send", input).await?, first);
    let history = call(&host, "alice", "read", json!({"conversation":conversation})).await?;
    assert_eq!(history["items"], json!([first]));
    Ok(())
}

#[tokio::test]
async fn paused_disabled_and_cross_conversation_replies_are_refused() -> Result<()> {
    let host = opened().await?;
    let a = register(&host, "alice", "A").await?;
    let b = register(&host, "alice", "B").await?;
    let first = call(&host, "alice", "open", json!({"members":[a,b]})).await?["id"]
        .as_u64()
        .unwrap();
    let second = call(&host, "alice", "open", json!({"members":[a,b]})).await?["id"]
        .as_u64()
        .unwrap();
    let original = call(
        &host,
        "alice",
        "send",
        json!({"conversation":first,"from":a,"to":b,"body":"First","key":"first"}),
    )
    .await?;
    let mut input = json!({"conversation":second,"from":b,"to":a,"body":"Reply","key":"reply","reply_to":original["id"]});
    assert!(call(&host, "alice", "send", input.clone()).await.is_err());
    input.as_object_mut().unwrap().remove("reply_to");
    call(
        &host,
        "alice",
        "pause",
        json!({"conversation":second,"paused":true}),
    )
    .await?;
    assert!(call(&host, "alice", "send", input.clone()).await.is_err());
    call(
        &host,
        "alice",
        "pause",
        json!({"conversation":second,"paused":false}),
    )
    .await?;
    call(
        &host,
        "alice",
        "update_agent",
        json!({"id":a,"enabled":false}),
    )
    .await?;
    assert!(call(&host, "alice", "send", input.clone()).await.is_err());
    call(
        &host,
        "alice",
        "update_agent",
        json!({"id":a,"enabled":true}),
    )
    .await?;
    call(&host, "alice", "send", input).await?;
    Ok(())
}

#[tokio::test]
async fn escaped_messages_fit_pages_and_cursors_survive_a_restart() -> Result<()> {
    let mut host = opened().await?;
    let a = register(&host, "alice", "A").await?;
    let b = register(&host, "alice", "B").await?;
    let conversation = call(&host, "alice", "open", json!({"members":[a,b]})).await?["id"]
        .as_u64()
        .unwrap();
    for key in 0..11 {
        call(&host, "alice", "send", json!({"conversation":conversation,"from":a,"to":b,"body":"\u{0001}".repeat(4096),"key":key.to_string()})).await?;
    }
    let page = call(&host, "alice", "read", json!({"conversation":conversation})).await?;
    assert_eq!(page["items"].as_array().unwrap().len(), 10);
    assert_eq!(page["has_more"], true);
    host.reopen().await?;
    let next = call(
        &host,
        "alice",
        "read",
        json!({"conversation":conversation,"after":page["next"]}),
    )
    .await?;
    assert_eq!(next["items"].as_array().unwrap().len(), 1);
    assert_eq!(next["has_more"], false);
    assert!(call(&host, "alice", "send", json!({"conversation":conversation,"from":a,"to":b,"body":"x".repeat(4097),"key":"oversize"})).await.is_err());
    Ok(())
}

#[tokio::test]
async fn committed_events_are_targeted_deduplicated_and_resumable() -> Result<()> {
    let mut host = opened().await?;
    let a = register(&host, "alice", "Sender").await?;
    let b = register(&host, "alice", "Recipient").await?;
    let c = register(&host, "alice", "Other").await?;
    let conversation = call(&host, "alice", "open", json!({"members":[a,b,c]})).await?["id"]
        .as_u64()
        .unwrap();
    let caller = Caller {
        id: "alice".into(),
        app: None,
        user: None,
        client: None,
        agent: None,
        key: None,
    };
    let initial = host
        .host
        .event_page(
            "relay",
            Some(&caller),
            "message.created",
            json!({"agent":b}),
            None,
        )
        .await?;
    assert!(initial.events.is_empty());
    let input = json!({"conversation":conversation,"from":a,"to":b,"body":"Once","key":"once"});
    let first = call(&host, "alice", "send", input.clone()).await?;
    call(&host, "alice", "send", input).await?;
    call(
        &host,
        "alice",
        "send",
        json!({"conversation":conversation,"from":a,"to":c,"body":"Other","key":"other"}),
    )
    .await?;
    assert!(
        call(
            &host,
            "alice",
            "send",
            json!({"conversation":conversation,"from":a,"to":b,"body":"","key":"failed"})
        )
        .await
        .is_err()
    );
    host.reopen().await?;
    let page = host
        .host
        .event_page(
            "relay",
            Some(&caller),
            "message.created",
            json!({"agent":b}),
            Some(&initial.cursor),
        )
        .await?;
    assert_eq!(page.events.len(), 1);
    assert_eq!(page.events[0].data, first);
    assert_eq!(page.events[0].id, first["id"].to_string());
    assert_eq!(page.events[0].at, first["created_at"].as_u64().unwrap());
    let tail = host
        .host
        .event_page(
            "relay",
            Some(&caller),
            "message.created",
            json!({"agent":b}),
            None,
        )
        .await?;
    assert!(tail.events.is_empty());
    assert_eq!(tail.cursor, page.cursor);
    let empty = host
        .host
        .event_page(
            "relay",
            Some(&caller),
            "message.created",
            json!({"agent":b}),
            Some(&page.cursor),
        )
        .await?;
    assert!(empty.events.is_empty());
    let bob = Caller {
        id: "bob".into(),
        ..caller.clone()
    };
    assert!(
        host.host
            .event_page(
                "relay",
                Some(&bob),
                "message.created",
                json!({"agent":b}),
                Some("0")
            )
            .await
            .is_err()
    );
    assert!(
        host.host
            .event_page("relay", None, "message.created", json!({"agent":b}), None)
            .await
            .is_err()
    );
    assert!(
        host.host
            .event_page(
                "relay",
                Some(&caller),
                "message.created",
                json!({"agent":b,"cursor":"0"}),
                None
            )
            .await
            .is_err()
    );
    assert!(
        host.host
            .event_page(
                "relay",
                Some(&caller),
                "message.created",
                json!({"agent":b}),
                Some("invalid")
            )
            .await
            .is_err()
    );
    call(
        &host,
        "alice",
        "update_agent",
        json!({"id":b,"enabled":false}),
    )
    .await?;
    assert!(
        host.host
            .event_page(
                "relay",
                Some(&caller),
                "message.created",
                json!({"agent":b}),
                Some("0")
            )
            .await
            .is_err()
    );
    let idl = host.host.idl("relay")?;
    let spec = idl
        .tools
        .iter()
        .find(|tool| tool.name == "message_events")
        .unwrap();
    assert!(spec.query);
    let source = Source::try_from(spec)?.qualify("relay");
    assert_eq!(source.definition.name, "relay.message.created");
    assert_eq!(source.definition.payload_schema["title"], "Message");
    assert!(
        source.definition.input_schema["properties"]
            .get("cursor")
            .is_none()
    );
    Ok(())
}

#[tokio::test]
async fn event_pages_can_resume_after_individual_acknowledgments() -> Result<()> {
    let mut host = opened().await?;
    let a = register(&host, "alice", "Sender").await?;
    let b = register(&host, "alice", "Recipient").await?;
    let conversation = call(&host, "alice", "open", json!({"members":[a,b]})).await?["id"]
        .as_u64()
        .unwrap();
    for key in 0..11 {
        call(&host, "alice", "send", json!({"conversation":conversation,"from":a,"to":b,"body":"\u{0001}".repeat(4096),"key":key.to_string()})).await?;
    }
    let page = call(
        &host,
        "alice",
        "message_events",
        json!({"agent":b,"cursor":"0"}),
    )
    .await?;
    assert_eq!(page["events"].as_array().unwrap().len(), 10);
    assert_eq!(page["has_more"], true);
    host.reopen().await?;
    let resumed = call(
        &host,
        "alice",
        "message_events",
        json!({"agent":b,"cursor":page["events"][2]["cursor"]}),
    )
    .await?;
    assert_eq!(resumed["events"].as_array().unwrap().len(), 8);
    assert_eq!(resumed["events"][0], page["events"][3]);
    assert_eq!(resumed["has_more"], false);
    Ok(())
}

#[tokio::test]
async fn paused_conversations_hold_the_event_position_until_resumed() -> Result<()> {
    let host = opened().await?;
    let from = register(&host, "alice", "Sender").await?;
    let to = register(&host, "alice", "Recipient").await?;
    let conversation =
        call(&host, "alice", "open", json!({"members":[from,to]})).await?["id"].clone();
    let message = call(
        &host,
        "alice",
        "send",
        json!({"conversation":conversation,"from":from,"to":to,"body":"Queued","key":"queued"}),
    )
    .await?;
    call(
        &host,
        "alice",
        "pause",
        json!({"conversation":conversation,"paused":true}),
    )
    .await?;
    let paused = call(
        &host,
        "alice",
        "message_events",
        json!({"agent":to,"cursor":"0"}),
    )
    .await?;
    assert_eq!(paused["events"], json!([]));
    assert_eq!(paused["cursor"], "0");
    call(
        &host,
        "alice",
        "pause",
        json!({"conversation":conversation,"paused":false}),
    )
    .await?;
    let resumed = call(
        &host,
        "alice",
        "message_events",
        json!({"agent":to,"cursor":paused["cursor"]}),
    )
    .await?;
    assert_eq!(resumed["events"][0]["data"], message);
    Ok(())
}

#[tokio::test]
async fn api_keys_act_only_as_granted_agents_and_revocation_survives_restart() -> Result<()> {
    let mut host = opened().await?;
    let a = register(&host, "alice", "Research").await?;
    let b = register(&host, "alice", "Assistant").await?;
    let c = register(&host, "alice", "Sales").await?;
    let conversation = call(&host, "alice", "open", json!({"members":[a,b]})).await?["id"].clone();
    let private = call(&host, "alice", "open", json!({"members":[b,c]})).await?["id"].clone();
    let input =
        json!({"conversation":conversation,"from":a,"to":b,"body":"Investigate","key":"first"});
    assert!(
        call_as(&host, "alice", Some("research-key"), "send", input.clone())
            .await
            .is_err()
    );
    call(
        &host,
        "alice",
        "grant_agent",
        json!({"agent":a,"key_id":"research-key"}),
    )
    .await?;
    call(
        &host,
        "alice",
        "grant_agent",
        json!({"agent":b,"key_id":"assistant-key"}),
    )
    .await?;
    assert_eq!(
        call(&host, "alice", "agent_keys", json!({"agent":a})).await?,
        json!(["research-key"])
    );
    let first = call_as(&host, "alice", Some("research-key"), "send", input.clone()).await?;
    assert_eq!(
        call_as(&host, "alice", Some("research-key"), "send", input.clone()).await?,
        first
    );
    assert_eq!(
        call_as(&host, "alice", Some("research-key"), "agents", json!({})).await?[0]["id"],
        a
    );
    assert!(
        call_as(
            &host,
            "alice",
            Some("research-key"),
            "inbox",
            json!({"agent":b})
        )
        .await
        .is_err()
    );
    assert!(
        call_as(
            &host,
            "alice",
            Some("research-key"),
            "message_events",
            json!({"agent":b,"cursor":"0"})
        )
        .await
        .is_err()
    );
    assert!(
        call_as(
            &host,
            "alice",
            Some("research-key"),
            "read",
            json!({"conversation":private})
        )
        .await
        .is_err()
    );
    assert!(
        call_as(
            &host,
            "bob",
            Some("research-key"),
            "inbox",
            json!({"agent":a})
        )
        .await
        .is_err()
    );
    let mut impersonated = input.clone();
    impersonated["from"] = json!(b);
    impersonated["to"] = json!(a);
    assert!(
        call_as(&host, "alice", Some("research-key"), "send", impersonated)
            .await
            .is_err()
    );
    for (tool, args) in [
        (
            "register",
            json!({"name":"Escalate","platform":{"open_ai_dot":{}}}),
        ),
        ("update_agent", json!({"id":a,"enabled":false})),
        ("grant_agent", json!({"agent":b,"key_id":"research-key"})),
        ("revoke_agent", json!({"agent":b,"key_id":"assistant-key"})),
        ("agent_keys", json!({"agent":b})),
        ("open", json!({"members":[a,c]})),
        ("pause", json!({"conversation":conversation,"paused":true})),
    ] {
        assert!(
            call_as(&host, "alice", Some("research-key"), tool, args)
                .await
                .is_err(),
            "{tool}"
        );
    }
    host.reopen().await?;
    assert_eq!(
        call_as(
            &host,
            "alice",
            Some("assistant-key"),
            "inbox",
            json!({"agent":b})
        )
        .await?["items"],
        json!([first.clone()])
    );
    assert_eq!(
        call_as(
            &host,
            "alice",
            Some("assistant-key"),
            "message_events",
            json!({"agent":b,"cursor":"0"})
        )
        .await?["events"][0]["data"],
        first
    );
    call(
        &host,
        "alice",
        "revoke_agent",
        json!({"agent":a,"key_id":"research-key"}),
    )
    .await?;
    host.reopen().await?;
    assert!(
        call_as(&host, "alice", Some("research-key"), "send", input)
            .await
            .is_err()
    );
    assert!(
        call_as(
            &host,
            "alice",
            Some("research-key"),
            "read",
            json!({"conversation":conversation})
        )
        .await
        .is_err()
    );
    assert_eq!(
        call_as(&host, "alice", Some("research-key"), "agents", json!({})).await?,
        json!([])
    );
    call_as(
        &host,
        "alice",
        Some("assistant-key"),
        "send",
        json!({"conversation":conversation,"from":b,"to":a,"body":"Found it","key":"reply"}),
    )
    .await?;
    Ok(())
}
