use crabtalk_sdk::services::service::UpgradePolicy;

#[test]
fn automatic_updates_require_an_opt_in_and_a_newer_stable_patch() {
    let mut policy = UpgradePolicy::default();
    assert!(!policy.accepts("1.2.3", "1.2.4"));
    policy.enabled = true;
    for (current, next, allowed) in [
        ("1.2.3", "1.2.4", true),
        ("0.1.1", "0.1.2", true),
        ("1.2.3", "1.2.3", false),
        ("1.2.3", "1.2.2", false),
        ("1.2.3", "1.3.0", false),
        ("1.2.3", "2.2.4", false),
        ("1.2.3", "1.2.4-beta.1", false),
        ("1.2.3-beta.1", "1.2.4", false),
        ("1.2.3", "1.2.3+rebuild", false),
        ("invalid", "1.2.4", false),
    ] {
        assert_eq!(
            policy.accepts(current, next),
            allowed,
            "{current} -> {next}"
        );
    }
}
