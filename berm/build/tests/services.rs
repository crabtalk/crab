//! Deployable service images and their use as linked dependencies.
use berm_api::Manifest;
use std::{fs, path::Path};

fn build(path: &str) -> Manifest {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let image = berm_build::build(&root.join(path), None).unwrap();
    Manifest::try_from(fs::read(image).unwrap().as_slice()).unwrap()
}

#[test]
fn provider_services_build_standalone_and_link_without_exporting_dependency_tools() {
    let resend = build("services/resend");
    assert_eq!(resend.tools.len(), 3);
    assert_eq!(resend.buffer, 1048576);
    assert_eq!(resend.deps.secrets, ["RESEND_API_KEY"]);
    assert!(!resend.deps.credentials["resend"].optional);
    let github = build("services/github");
    assert_eq!(github.tools.len(), 11);
    assert!(github.deps.credentials["github_api"].optional);
    let upstash = build("services/upstash");
    assert_eq!(upstash.tools.len(), 8);
    assert!(upstash.deps.vars.contains_key("UPSTASH_REDIS_REST_URL"));
    assert!(!upstash.deps.credentials["upstash_redis"].optional);
    let posthog = build("services/posthog");
    assert_eq!(posthog.tools.len(), 4);
    assert_eq!(posthog.deps.vars["POSTHOG_REGION"].default, "us");
    assert!(posthog.deps.credentials["posthog_project"].optional);
    assert!(posthog.deps.credentials["posthog_private"].optional);
    let capture = posthog.tools.iter().find(|t| t.name == "capture").unwrap();
    let schema = &capture.parameters;
    assert_eq!(
        schema["properties"]["event"]["properties"]["properties"]["additionalProperties"],
        serde_json::json!({})
    );
    let linked = build("fixtures/linked-services");
    assert_eq!(
        linked
            .tools
            .iter()
            .map(|t| t.name.as_str())
            .collect::<Vec<_>>(),
        ["inspect"]
    );
    assert!(linked.deps.secrets.iter().any(|s| s == "RESEND_API_KEY"));
    assert!(linked.deps.secrets.iter().any(|s| s == "GITHUB_TOKEN"));
    assert!(linked.deps.vars.is_empty());
    for credential in ["upstash_redis", "posthog_project", "posthog_private"] {
        assert!(linked.deps.credentials.contains_key(credential));
    }
    assert!(
        linked
            .deps
            .secrets
            .iter()
            .any(|s| s == "UPSTASH_REDIS_REST_TOKEN")
    );
    assert!(
        linked
            .deps
            .secrets
            .iter()
            .any(|s| s == "POSTHOG_PROJECT_TOKEN")
    );
    assert_eq!(linked.buffer, berm_api::DEFAULT_BUFFER);
    assert!(linked.deps.credentials.contains_key("github"));
    assert!(linked.deps.credentials.contains_key("github_api"));
    assert!(linked.keeps_database());
    let required = linked.deps.unset(&[]);
    assert!(required.iter().any(|name| name == "GITHUB_CLIENT_ID"));
    assert!(required.iter().any(|name| name == "GITHUB_CLIENT_SECRET"));
    assert!(!required.iter().any(|name| name == "GITHUB_TOKEN"));
    let hosts = &linked.deps.syscalls["berm.http.fetch"];
    assert!(hosts.iter().any(|s| s == "api.github.com"));
    assert!(hosts.iter().any(|s| s == "api.resend.com"));
}
