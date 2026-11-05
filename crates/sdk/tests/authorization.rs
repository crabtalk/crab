use crabtalk_sdk::account::request::{AgentLoginRequest, DeviceToken};
use serde_json::json;

#[test]
fn device_credentials_distinguish_agent_identity_from_legacy_sessions() {
    let legacy: DeviceToken = serde_json::from_value(json!({"token":"session"})).unwrap();
    assert!(legacy.account.is_none());
    let agent: DeviceToken = serde_json::from_value(json!({
        "token":"ck_agent",
        "account":{"id":"alice","type":"user","handle":"alice","name":"Alice","avatar":null,"bio":null,"links":[],"created_at":1}
    })).unwrap();
    assert_eq!(agent.account.unwrap().id, "alice");
    assert_eq!(
        serde_json::to_value(AgentLoginRequest {
            client_name: "Claude Code".into()
        })
        .unwrap(),
        json!({"client_name":"Claude Code"})
    );
}
