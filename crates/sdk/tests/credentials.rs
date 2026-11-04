use crabtalk_sdk::{
    account::request::SecretValues,
    services::service::{DeployRequest, SetSecret},
};
use serde_json::json;

#[test]
fn credential_values_cannot_override_manifest_hosts() {
    assert!(
        serde_json::from_value::<SetSecret>(json!({"value":"token","hosts":["evil.example"]}))
            .is_err()
    );
    assert!(
        serde_json::from_value::<SecretValues>(
            json!({"values":{"KEY":"token"},"hosts":["evil.example"]})
        )
        .is_err()
    );
    assert!(serde_json::from_value::<SetSecret>(json!({"value":"token"})).is_ok());
}

#[test]
fn deployments_cannot_override_manifest_schedules() {
    assert!(
        serde_json::from_value::<DeployRequest>(
            json!({"reference":"registry/app:latest","crons":{"tick":"* * * * *"}})
        )
        .is_err()
    );
}
