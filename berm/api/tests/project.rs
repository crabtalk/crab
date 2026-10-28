//! `crab.toml` credentials: parsed, folded into `deps`, and a secret declared
//! twice refused.

use berm_api::project::Project;
use std::fs;

fn read(toml: &str) -> anyhow::Result<Project> {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("crab.toml");
    fs::write(&path, toml).unwrap();
    Project::read(&path)
}

const GITHUB: &str = r#"
name = "app"
secrets = ["OPENAI_KEY"]

[credentials.github]
title = "GitHub OAuth app"
guide = "Create one."
copy = { callback = "Authorization callback URL" }
secrets = [
    { name = "GITHUB_CLIENT_ID", label = "Client ID", masked = false, pattern = '^Ov23li' },
    { name = "GITHUB_CLIENT_SECRET", label = "Client secret" },
]
"#;

#[test]
fn a_credentials_secrets_join_the_declared_secrets() {
    let deps = read(GITHUB).unwrap().deps();
    assert_eq!(
        deps.secrets,
        ["OPENAI_KEY", "GITHUB_CLIENT_ID", "GITHUB_CLIENT_SECRET"]
    );
    let github = &deps.credentials["github"];
    assert_eq!(github.copy["callback"], "Authorization callback URL");
    assert!(!github.secrets[0].masked);
    assert!(github.secrets[1].masked);
    assert_eq!(github.secrets[0].pattern.as_deref(), Some("^Ov23li"));
}

#[test]
fn a_secret_declared_twice_is_refused() {
    let twice = GITHUB.replace(r#"["OPENAI_KEY"]"#, r#"["GITHUB_CLIENT_ID"]"#);
    let error = read(&twice).unwrap_err();
    assert!(
        error.to_string().contains("GITHUB_CLIENT_ID twice"),
        "{error}"
    );
}

#[test]
fn an_unknown_key_in_a_credential_is_refused() {
    let typo = GITHUB.replace("masked = false", "masekd = false");
    assert!(read(&typo).is_err());
}

#[test]
fn the_auth_kits_credentials_parse() {
    let kit = concat!(env!("CARGO_MANIFEST_DIR"), "/../../kits/auth");
    let project = Project::read(&std::path::Path::new(kit).join("crab.toml")).unwrap();
    project.check_kit().unwrap();
    let deps = project.deps();
    assert_eq!(deps.credential_of("GOOGLE_CLIENT_SECRET"), Some("google"));
    assert_eq!(deps.secrets.len(), 4);
}

#[test]
fn a_programs_prices_are_read_and_a_kits_are_refused() {
    let priced = read("name = \"app\"\n\n[prices]\nsearch = \"0.002\"\n").unwrap();
    assert_eq!(priced.prices["search"], "0.002");
    let kit = read("[prices]\nsearch = \"0.002\"\n").unwrap();
    assert!(kit.check_kit().unwrap_err().to_string().contains("prices"));
}
