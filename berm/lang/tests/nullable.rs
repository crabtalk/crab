use crate::nullable::{calls, types::Entry};
use berm_lang::{CallError, test};
use std::collections::BTreeMap;

berm_lang::declare_service!(nullable);

#[test]
fn generated_calls_decode_nullable_returns_and_nested_values() {
    test::answer("nullable", "recall", Ok(b"null"));
    let recalled: Result<Option<Entry>, CallError> = calls::recall();
    assert!(recalled.unwrap().is_none());
    test::answer("nullable", "recall", Ok(br#"{"key":"a"}"#));
    let recalled = calls::recall().unwrap().unwrap();
    assert_eq!(recalled.key, "a");
    assert!(recalled.note.is_none());

    test::answer("nullable", "search", Ok(b"[1,null]"));
    let found: Vec<Option<i64>> = calls::search().unwrap();
    assert_eq!(found, [Some(1), None]);
    test::answer("nullable", "entries", Ok(br#"{"a":true,"b":null}"#));
    let entries: BTreeMap<String, Option<bool>> = calls::entries().unwrap();
    assert_eq!(entries["a"], Some(true));
    assert_eq!(entries["b"], None);
}
