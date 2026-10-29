//! A secret's value against the pattern its credential declares.

use berm_api::{Credential, CredentialSecret, Deps};
use std::collections::BTreeMap;

fn deps(pattern: Option<&str>) -> Deps {
    let credential = Credential {
        title: "Stripe restricted key".to_owned(),
        guide: String::new(),
        copy: BTreeMap::new(),
        hosts: Vec::new(),
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
