use berm_api::schema::{self, Validator};
use serde_json::json;

#[test]
fn reusable_validators_preserve_results_and_errors() {
    let schema = json!({"type":"object","properties":{"count":{"type":"integer","minimum":1}},"required":["count"],"additionalProperties":false});
    let validator = Validator::try_from(&schema).unwrap();
    for value in [
        json!({"count":1}),
        json!({"count":100}),
        json!({"count":0}),
        json!({"count":"one"}),
        json!({}),
        json!({"count":1,"other":true}),
    ] {
        assert_eq!(
            validator.validate(&value).map_err(|e| e.to_string()),
            schema::validate(&schema, &value).map_err(|e| e.to_string())
        );
    }
    assert!(Validator::try_from(&json!({"type":"invalid"})).is_err());
}
