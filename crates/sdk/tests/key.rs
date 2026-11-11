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
    for text in [
        "deploy",
        "auto",
        "services:delete",
        "deployments:request",
        "secrets:write",
        "schedules:read",
        "schedules:write",
    ] {
        assert!(text.parse::<Scope>().is_err());
    }
}

#[test]
fn omitted_permissions_use_defaults_and_explicit_empty_permissions_stay_empty() {
    let default: CreateKeyRequest = serde_json::from_value(json!({"label":"agent"})).unwrap();
    assert_eq!(default.scope, Scope::DEFAULT);
    assert!(!default.scope.contains(&Scope::SecretsRead));
    let empty: CreateKeyRequest =
        serde_json::from_value(json!({"label":"agent","scope":[]})).unwrap();
    assert!(empty.scope.is_empty());
}

#[test]
fn write_permissions_include_only_their_own_resource_reads() {
    for (write, read) in [
        (Scope::ServicesWrite, Scope::ServicesRead),
        (Scope::DeploymentsWrite, Scope::DeploymentsRead),
        (Scope::AppsWrite, Scope::AppsRead),
        (Scope::OrgsWrite, Scope::OrgsRead),
        (Scope::MembersWrite, Scope::MembersRead),
        (Scope::InvitesWrite, Scope::InvitesRead),
        (Scope::TeamsWrite, Scope::TeamsRead),
        (Scope::AlertsWrite, Scope::AlertsRead),
    ] {
        for required in Scope::ALL {
            assert_eq!(
                write.allows(required),
                required == write || required == read
            );
            assert_eq!(read.allows(required), required == read);
        }
    }
    for scope in [
        Scope::CallsRead,
        Scope::LogsRead,
        Scope::UsageRead,
        Scope::BillingRead,
        Scope::EarningsRead,
        Scope::KeysRead,
        Scope::SecretsRead,
    ] {
        for required in Scope::ALL {
            assert_eq!(scope.allows(required), scope == required);
        }
    }
}

#[test]
fn key_creation_rejects_unusable_names_and_negative_budgets() {
    for body in [
        json!({"label":"   "}),
        json!({"label":"x".repeat(101)}),
        json!({"label":"agent","budget":-1}),
    ] {
        let settings: CreateKeyRequest = serde_json::from_value(body).unwrap();
        assert!(settings.validate().is_err());
    }
    let settings: CreateKeyRequest =
        serde_json::from_value(json!({"label":" Agent ","budget":0,"scope":[]})).unwrap();
    assert!(settings.validate().is_ok());
}
