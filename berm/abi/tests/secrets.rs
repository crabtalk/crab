use berm_abi::secrets::Id;

#[test]
fn resource_ids_accept_issued_and_existing_hex_lengths() {
    for length in [32, 64] {
        let id = format!("secret_{}", "a1".repeat(length / 2));
        assert_eq!(Id::try_from(id.as_str()).unwrap().0, id);
    }
}

#[test]
fn resource_ids_reject_other_lengths_and_non_lowercase_hex() {
    for length in [0, 31, 33, 63, 65] {
        let id = format!("secret_{}", "a".repeat(length));
        assert!(Id::try_from(id.as_str()).is_err());
    }
    for id in [
        "secret_AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
        "secret_gggggggggggggggggggggggggggggggg",
        "other_11111111111111111111111111111111",
    ] {
        assert!(Id::try_from(id).is_err());
    }
}
