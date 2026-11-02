use crabtalk_sdk::account::key::{CreateKeyRequest, Scope};
use serde_json::json;

#[test]
fn scopes_round_trip_and_exclude_legacy_and_deletion_permissions() {
    for scope in Scope::ALL {
        let text = scope.to_string();
        assert_eq!(text.parse::<Scope>().unwrap(), scope);
        assert_eq!(serde_json::from_value::<Scope>(json!(text)).unwrap(), scope);
        assert!(!text.ends_with(":delete"));
    }
    for text in ["deploy", "auto", "services:delete", "deployments:request"] {
        assert!(text.parse::<Scope>().is_err());
    }
}

#[test]
fn omitted_permissions_use_defaults_and_explicit_empty_permissions_stay_empty() {
    let default: CreateKeyRequest = serde_json::from_value(json!({"label":"agent"})).unwrap();
    assert_eq!(default.scope, Scope::DEFAULT);
    let empty: CreateKeyRequest =
        serde_json::from_value(json!({"label":"agent","scope":[]})).unwrap();
    assert!(empty.scope.is_empty());
}
