use berm_api::reference::Reference;

#[test]
fn qualified_image_addresses_round_trip_with_optional_exact_versions() {
    for address in [
        "connect/tinyfish",
        "alice/weather:1.2.3",
        "alice/weather:2.0.0-rc.1",
    ] {
        let reference: Reference = address.parse().unwrap();
        assert_eq!(reference.to_string(), address);
    }
    let reference: Reference = "connect/tinyfish".parse().unwrap();
    assert_eq!(reference.owner, "connect");
    assert_eq!(reference.name, "tinyfish");
    assert!(reference.version.is_none());
}

#[test]
fn malformed_addresses_and_version_ranges_are_rejected() {
    for address in [
        "tinyfish",
        "/tinyfish",
        "connect/",
        "connect/tiny/fish",
        "Connect/tinyfish",
        "connect/tinyfish:latest",
        "connect/tinyfish:^1.2",
        "connect/tinyfish:",
    ] {
        assert!(address.parse::<Reference>().is_err(), "{address}");
    }
}
