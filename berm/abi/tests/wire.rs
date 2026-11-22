use berm_abi::wire;

#[test]
fn http_refusal_adds_an_optional_explanation_field() {
    assert_eq!(
        wire::fields(&wire::http_refusal("blocked", None)).unwrap(),
        vec![&b"refused"[..], b"blocked"],
    );
    assert_eq!(
        wire::fields(&wire::http_refusal("blocked", Some("E0091"))).unwrap(),
        vec![&b"refused"[..], b"blocked", b"E0091"],
    );
}
