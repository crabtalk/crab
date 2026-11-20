use crabtalk_sdk::services::service::DeployRequest;
use serde_json::json;
#[test]
fn deploy_overrides_round_trip_and_retired_template_keys_are_refused() {
    let request: DeployRequest = serde_json::from_value(
        json!({"reference":"alice/weather:1.0.0","public":false,"prices":{"echo":0}}),
    )
    .unwrap();
    assert_eq!(request.overrides.public, Some(false));
    assert_eq!(request.overrides.prices["echo"], 0);
    for key in ["template", "source", "release_notes"] {
        assert!(
            serde_json::from_value::<DeployRequest>(
                json!({"reference":"alice/weather",(key):true})
            )
            .is_err(),
            "{key}"
        );
    }
}
