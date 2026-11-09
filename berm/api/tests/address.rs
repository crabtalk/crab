use berm_api::address::Address;

#[test]
fn service_addresses_are_qualified_and_never_image_versions() {
    let address: Address = "alice/weather".parse().unwrap();
    assert_eq!(address.to_string(), "alice/weather");
    for invalid in [
        "weather",
        "/weather",
        "alice/",
        "alice/weather:1.0.0",
        "alice/../weather",
        "Alice/weather",
    ] {
        assert!(invalid.parse::<Address>().is_err(), "{invalid}");
    }
}
