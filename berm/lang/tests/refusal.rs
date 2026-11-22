use berm_lang::{CallError, call, test};

#[test]
fn legacy_refusals_keep_their_message_and_expose_the_host_code() {
    let message = r#"{"status":403,"error":"blocked","explain":"E0091"}: blocked"#;
    test::answer("inner", "fetch", Err(CallError::Refused(message.into())));
    let error = call("inner", "fetch", "{}").unwrap_err();
    assert!(matches!(error, CallError::Refused(_)));
    assert_eq!(error.message(), message);
    assert_eq!(error.explanation(), Some("E0091"));

    test::answer("inner", "fetch", Err(CallError::Refused("missing".into())));
    let error = call("inner", "fetch", "{}").unwrap_err();
    assert!(error.refused());
    assert_eq!(error.message(), "missing");
    assert_eq!(error.explanation(), None);
}
