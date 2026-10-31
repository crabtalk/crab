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
