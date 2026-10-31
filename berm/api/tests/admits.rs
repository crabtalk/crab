//! A secret's value against the pattern its credential declares.

use berm_api::{Credential, CredentialSecret, Deps};
use std::collections::BTreeMap;

fn deps(pattern: Option<&str>) -> Deps {
    let credential = Credential {
        title: "Stripe restricted key".to_owned(),
        guide: String::new(),
        copy: BTreeMap::new(),
        hosts: Vec::new(),
        optional: false,
        secrets: vec![CredentialSecret {
            name: "STRIPE_KEY".to_owned(),
            label: "Restricted key".to_owned(),
            masked: true,
            pattern: pattern.map(str::to_owned),
        }],
    };
    Deps {
        secrets: vec!["STRIPE_KEY".to_owned(), "LOOSE".to_owned()],
        credentials: BTreeMap::from([("stripe".to_owned(), credential)]),
        ..Deps::default()
    }
}

#[test]
fn a_value_its_pattern_takes_is_admitted() {
    assert!(
        deps(Some("^rk_"))
            .admits("STRIPE_KEY", "rk_live_123")
            .is_ok()
    );
}

#[test]
fn a_value_its_pattern_refuses_is_refused_by_its_label() {
    let error = deps(Some("^rk_"))
        .admits("STRIPE_KEY", "sk_live_123")
        .unwrap_err();
    assert!(error.contains("Restricted key"), "{error}");
}

#[test]
fn a_secret_with_no_pattern_takes_any_value() {
    assert!(deps(None).admits("STRIPE_KEY", "anything").is_ok());
    assert!(deps(Some("^rk_")).admits("LOOSE", "anything").is_ok());
}

#[test]
fn unset_includes_only_missing_required_secrets() {
    let mut deps = deps(None);
    deps.secrets.push("OPTIONAL".to_owned());
    let mut optional = deps.credentials["stripe"].clone();
    optional.optional = true;
    optional.secrets[0].name = "OPTIONAL".to_owned();
    deps.credentials.insert("optional".to_owned(), optional);
    assert_eq!(deps.unset(&["STRIPE_KEY".to_owned()]), ["LOOSE"]);
}
