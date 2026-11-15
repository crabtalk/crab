use berm_api::{Access, ToolSpec, host::Caller};
use serde_json::json;

#[test]
fn tool_access_distinguishes_accounts_anonymous_apps_and_services() {
    let account: Caller = serde_json::from_value(json!({"id": "alice"})).unwrap();
    let app: Caller = serde_json::from_value(json!({"id": "alice", "app": "site"})).unwrap();
    let user: Caller = serde_json::from_value(json!({
        "id": "alice", "app": "site", "user": {"issuer": "https://id.example", "subject": "42"}
    }))
    .unwrap();
    let mut tool: ToolSpec = serde_json::from_value(json!({
        "name": "echo", "description": "", "parameters": {}
    }))
    .unwrap();
    for (access, expected) in [
        (None, [false, false, false, false]),
        (Some(Access::Services), [false, false, false, false]),
        (Some(Access::Anonymous), [true, true, true, true]),
        (Some(Access::Public), [false, true, false, true]),
        (Some(Access::Accounts), [false, true, false, true]),
    ] {
        tool.set_access(access);
        let actual =
            [None, Some(&account), Some(&app), Some(&user)].map(|caller| tool.callable_by(caller));
        assert_eq!(actual, expected, "{access:?}");
    }
}
