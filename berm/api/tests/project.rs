//! `crab.toml`: credentials parsed, folded into `deps`, and a secret declared
//! twice refused; `[tools]` read and checked.

use berm_api::{Access, project::Project};
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
fn a_programs_tools_are_read_and_a_kits_are_refused() {
    let program = read(
        "name = \"app\"\n\n[tools.search]\naccess = \"services\"\ntitle = \"Search\"\nprice = 2_000\ncatalog = \"web.search\"\n",
    )
    .unwrap();
    let search = &program.tools["search"];
    assert_eq!(search.access, Some(Access::Services));
    assert_eq!(search.price, Some(2_000));
    assert_eq!(
        program.records()["search"].catalog.as_deref(),
        Some("web.search")
    );
    let kit = read("[tools.search]\naccess = \"public\"\n").unwrap();
    assert!(kit.check_kit().unwrap_err().to_string().contains("tools"));
}

#[test]
fn a_price_or_catalog_on_an_internal_tool_is_refused() {
    for key in ["price = 2_000", "catalog = \"web.search\""] {
        let error = read(&format!("name = \"app\"\n[tools.search]\n{key}\n")).unwrap_err();
        assert!(error.to_string().contains("internal tool"), "{error}");
    }
}

#[test]
fn a_title_past_sixty_characters_or_one_line_is_refused() {
    let long = "x".repeat(61);
    for title in [long.as_str(), "two\\nlines"] {
        let error = read(&format!(
            "name = \"app\"\n[tools.search]\ntitle = \"{title}\"\n"
        ))
        .unwrap_err();
        assert!(error.to_string().contains("one line"), "{error}");
    }
    read(&format!(
        "name = \"app\"\n[tools.search]\ntitle = \"{}\"\n",
        "é".repeat(60)
    ))
    .unwrap();
}

#[test]
fn an_unknown_access_or_key_is_refused() {
    assert!(read("name = \"app\"\n[tools.search]\naccess = \"everyone\"\n").is_err());
    assert!(read("name = \"app\"\n[tools.search]\nprices = 1\n").is_err());
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
