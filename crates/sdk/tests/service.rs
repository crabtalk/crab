use crabtalk_sdk::services::service::DeployRequest;
use serde_json::json;

#[test]
fn deployments_reject_visibility_overrides() {
    for public in [true, false] {
        assert!(
            serde_json::from_value::<DeployRequest>(json!({
                "reference": "sha256:image", "public": public
            }))
            .is_err()
        );
    }
}
