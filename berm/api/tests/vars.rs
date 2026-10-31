//! `[vars]` in `crab.toml`: read into `deps` with `berm.var` granted, checked
//! by name, size and count, and refused in a kit.

use berm_api::{Var, project::Project};
use std::fs;

fn read(toml: &str) -> anyhow::Result<Project> {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("crab.toml");
    fs::write(&path, toml).unwrap();
    Project::read(&path)
}

const WATCH: &str = r#"
name = "watch"

[vars]
keywords = { default = "rust, wasm", description = "Words to watch" }
"#;

#[test]
fn vars_join_the_deps_and_grant_berm_var() {
    let deps = read(WATCH).unwrap().deps();
    let keywords = &deps.vars["keywords"];
    assert_eq!(keywords.default, "rust, wasm");
    assert_eq!(keywords.description, "Words to watch");
    assert!(deps.syscalls.contains_key("berm.var"));
}

#[test]
fn a_service_with_no_vars_is_granted_no_berm_var() {
    let deps = read("name = \"plain\"\n").unwrap().deps();
    assert!(deps.vars.is_empty());
    assert!(!deps.syscalls.contains_key("berm.var"));
}

#[test]
fn a_var_name_past_letters_digits_and_underscores_is_refused() {
    let error = read(&WATCH.replace("keywords =", "\"key-words\" =")).unwrap_err();
    assert!(error.to_string().contains("vars.key-words"), "{error}");
}

#[test]
fn a_default_past_its_bytes_is_refused() {
    let long = WATCH.replace("rust, wasm", &"x".repeat(Var::BYTES + 1));
    let error = read(&long).unwrap_err();
    assert!(error.to_string().contains("default is past"), "{error}");
}

#[test]
fn more_vars_than_the_most_are_refused() {
    let mut toml = String::from("name = \"many\"\n[vars]\n");
    for at in 0..=Var::MAX {
        toml.push_str(&format!("v{at} = {{}}\n"));
    }
    let error = read(&toml).unwrap_err();
    assert!(error.to_string().contains("vars, past the"), "{error}");
}

#[test]
fn a_kit_declaring_vars_is_refused() {
    let kit = read("[vars]\nkeywords = {}\n").unwrap();
    let error = kit.check_kit().unwrap_err();
    assert!(error.to_string().contains("not vars"), "{error}");
}

#[test]
fn settable_refuses_an_undeclared_var_and_a_long_value() {
    let deps = read(WATCH).unwrap().deps();
    deps.settable("keywords", "go").unwrap();
    assert!(deps.settable("missing", "go").is_err());
    assert!(
        deps.settable("keywords", &"x".repeat(Var::BYTES + 1))
            .is_err()
    );
}
