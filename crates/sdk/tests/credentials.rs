use berm_api::Deps;
use crabtalk_sdk::{
    account::{request::SecretValues, secret::Grant},
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

#[test]
fn runtime_secret_grants_require_capability_and_existing_http_permissions() {
    let grant = Grant {
        service: "relay".into(),
        hosts: vec!["api.example.com".into()],
    };
    let mut deps = Deps::default();
    assert!(grant.check(&deps).is_err());
    deps.secret_resources = true;
    assert!(grant.check(&deps).is_err());
    deps.http.fetch = vec!["api.example.com".into()];
    assert!(grant.check(&deps).is_ok());
    assert!(
        Grant {
            service: "relay".into(),
            hosts: vec!["evil.example".into()]
        }
        .check(&deps)
        .is_err()
    );
}

#[test]
fn resource_ids_cannot_be_provisioned_as_default_credentials() {
    let input = SetSecret {
        value: "key".into(),
    };
    assert!(
        input
            .check("secret_11111111111111111111111111111111")
            .is_err()
    );
}
