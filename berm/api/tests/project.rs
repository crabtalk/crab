//! `crab.toml`: credentials parsed, folded into `deps`, and a secret declared
//! twice refused; `[tools]` read and checked.

use berm_api::project::Project;
use std::fs;

fn read(toml: &str) -> anyhow::Result<Project> {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("crab.toml");
    fs::write(&path, toml).unwrap();
    Project::read(&path)
}

const GITHUB: &str = r#"
image = "fixtures/app"
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
    let kit = concat!(env!("CARGO_MANIFEST_DIR"), "/../../services/kits/auth");
    let project = Project::read(&std::path::Path::new(kit).join("crab.toml")).unwrap();
    project.check_kit().unwrap();
    let deps = project.deps();
    assert_eq!(deps.credential_of("GOOGLE_CLIENT_SECRET"), Some("google"));
    assert_eq!(deps.secrets.len(), 4);
}

#[test]
fn an_unknown_access_or_key_is_refused() {
    assert!(read("image = \"fixtures/app\"\n[tools.search]\naccess = \"everyone\"\n").is_err());
    assert!(read("image = \"fixtures/app\"\n[tools.search]\nprices = 1\n").is_err());
}

#[test]
fn a_pattern_that_is_not_a_regular_expression_is_refused() {
    let broken = GITHUB.replace("pattern = '^Ov23li'", "pattern = '^(Ov23li'");
    let error = read(&broken).unwrap_err();
    assert!(
        error.to_string().contains("not a regular expression"),
        "{error}"
    );
}

#[test]
fn linked_services_validate_migration_identity_without_rejecting_deployment_settings() {
    let service = read("image = \"fixtures/github\"\n[vars]\nlimit = {}\n").unwrap();
    service.check_dependency().unwrap();
    let missing = read("[database]\nmigrations = \"migrations\"\n").unwrap();
    assert!(missing.check_dependency().is_err());
}

#[test]
fn stateful_services_can_deploy_and_link_with_a_stable_migration_identity() {
    let service = read(
        r#"image = "fixtures/watch"
id = "732e70b4fd594e91a94da0f60f50a948"
[database]
migrations = "migrations"
"#,
    )
    .unwrap();
    assert_eq!(service.instance("package"), "watch");
    service.check_dependency().unwrap();
    assert!(
        read(
            r#"image = "fixtures/watch"
id = "invalid"
"#
        )
        .unwrap()
        .check_service()
        .is_err()
    );
}

#[test]
fn publication_is_opt_in_and_independent() {
    let private = read(r#"image = "fixtures/app""#).unwrap();
    assert!(!private.public && !private.template);
    for (public, template) in [(false, true), (true, false), (true, true)] {
        let project = read(&format!(
            "image = \"fixtures/app\"\npublic = {public}\ntemplate = {template}\n"
        ))
        .unwrap();
        assert_eq!((project.public, project.template), (public, template));
    }
}

#[test]
fn logos_are_optional_and_require_absolute_https_urls() {
    assert!(read("image = \"fixtures/app\"\n").unwrap().logo.is_none());
    let logo = read("image = \"fixtures/app\"\nlogo = \"https://example.com/logo.svg\"\n")
        .unwrap()
        .logo
        .unwrap();
    assert_eq!(logo.as_str(), "https://example.com/logo.svg");
    for url in [
        "http://example.com/logo.svg",
        "logo.svg",
        "//example.com/logo.svg",
        "https://",
        "data:image/png;base64,AA==",
        "",
    ] {
        let error = read(&format!("image = \"fixtures/app\"\nlogo = {url:?}\n")).unwrap_err();
        assert!(format!("{error:#}").contains("logo must be an absolute HTTPS URL"));
    }
}

#[test]
fn library_only_packages_do_not_declare_logos() {
    let project = read("logo = \"https://example.com/logo.svg\"\n").unwrap();
    assert!(
        project
            .check_kit()
            .unwrap_err()
            .to_string()
            .contains("logo")
    );
}

#[test]
fn http_settings_replace_syscall_declarations() {
    let project = read(
        r#"image = "fixtures/http"
[http]
fetch = ["api.example"]
proxy = ["*.example"]
"#,
    )
    .unwrap();
    assert_eq!(project.deps().http.fetch, ["api.example"]);
    assert_eq!(project.deps().http.proxy, ["*.example"]);
    assert!(
        read(
            r#"image = "fixtures/old"
[syscalls]
"berm.vouch" = []
"#
        )
        .is_err()
    );
    assert!(
        read(
            r#"image = "fixtures/typo"
[http]
ftech = []
"#
        )
        .is_err()
    );
    assert!(
        read(r#"image = "fixtures/plain""#)
            .unwrap()
            .deps()
            .http
            .is_empty()
    );
}

#[test]
fn builds_need_no_image_and_the_old_name_key_is_rejected() {
    let project = read("").unwrap();
    assert!(project.image.is_none());
    assert_eq!(project.instance("local-package"), "local-package");
    assert!(read("name = \"old-name\"").is_err());
    let project = read("image = \"connect/tinyfish\"").unwrap();
    assert_eq!(project.instance("crab-tinyfish"), "tinyfish");
}

#[test]
fn service_call_destinations_are_not_manifest_declarations() {
    let error = read("calls = [\"alice/weather\"]").unwrap_err();
    assert!(format!("{error:#}").contains("calls"), "{error:#}");
}

#[test]
fn legacy_declarations_are_refused_even_when_empty_or_equal_to_defaults() {
    for source in [
        r#"[tools]"#,
        r#"[tools.echo]"#,
        r#"[tools.echo]
access = "public""#,
        r#"[workflows.echo]
access = "public""#,
        r#"[workflows.echo]
max_cost = 100000"#,
        r#"[workflows.echo]
checkpoint_bytes = 8388608"#,
        r#"[workflows.echo]
retain = "7d""#,
        r#"[workflows.echo]
price = 100"#,
    ] {
        assert!(read(source).is_err(), "{source}");
    }
    read(
        r#"[workflows.echo.schedule]
cron = "0 8 * * *"
input = {}
"#,
    )
    .unwrap();
}
