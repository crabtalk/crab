use crabtalk_sdk::account::{
    auth::Status,
    key::Scope,
    request::{AgentLoginRequest, DeviceToken},
};
use serde_json::json;

#[test]
fn device_credentials_distinguish_agent_identity_from_personal_sessions() {
    let personal: DeviceToken = serde_json::from_value(json!({"token":"session"})).unwrap();
    assert!(personal.account.is_none());
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

#[test]
fn credential_status_checks_scope_inheritance_without_key_list_permission() {
    let status: Status = serde_json::from_value(json!({"kind":"api_key","key":{
        "id":"agent","owner_id":"org","label":"agent","scope":["deployments:write"],
        "expires_at":null,"budget":0,"spent":0,"services":[]
    }}))
    .unwrap();
    assert!(status.allows(Scope::DeploymentsWrite));
    assert!(status.allows(Scope::DeploymentsRead));
    assert!(!status.allows(Scope::KeysRead));
    assert!(
        !Status::Local {
            caller: Some("operator".into())
        }
        .allows(Scope::DeploymentsWrite)
    );
}
