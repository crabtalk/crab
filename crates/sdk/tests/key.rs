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
        "secrets:read",
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
    ] {
        for required in Scope::ALL {
            assert_eq!(scope.allows(required), scope == required);
        }
    }
}
