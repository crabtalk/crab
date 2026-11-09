//! Service selection by directory, explicit name and deployment eligibility.

use berm_build::Workspace;
use std::{fs, path::Path};

/// Write `text` to `path` under `root`, creating its directories.
fn write(root: &Path, path: &str, text: &str) {
    let path = root.join(path);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

/// A crate `name` in `root/name` whose `crab.toml` is `berm`.
fn service(root: &Path, name: &str, berm: &str) {
    write(
        root,
        &format!("{name}/Cargo.toml"),
        &format!("[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"2024\"\n"),
    );
    write(root, &format!("{name}/src/lib.rs"), "");
    write(root, &format!("{name}/crab.toml"), berm);
}

#[test]
fn a_service_saying_deploy_false_is_left_out_unless_named() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    write(
        &root,
        "Cargo.toml",
        "[workspace]\nmembers = [\"shown\", \"held\"]\nresolver = \"3\"\n",
    );
    service(&root, "shown", "image = \"fixtures/shown\"\n");
    service(&root, "held", "image = \"fixtures/held\"\ndeploy = false\n");

    let workspace = Workspace::read(&root, &[], false).unwrap();
    assert_eq!(workspace.services, [root.join("shown")]);
    assert_eq!(workspace.skipped, ["held"]);
    let publishing = Workspace::read(&root, &[], true).unwrap();
    assert_eq!(publishing.services.len(), 2);
    assert!(publishing.skipped.is_empty());

    let named = Workspace::read(&root.join("held"), &[], false).unwrap();
    assert_eq!(named.services, [root.join("held")]);
    assert!(named.skipped.is_empty());
}

#[test]
fn named_services_are_picked_from_anywhere_in_the_workspace() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    write(
        &root,
        "Cargo.toml",
        "[workspace]\nmembers = [\"one\", \"two\", \"held\"]\nresolver = \"3\"\n",
    );
    service(&root, "one", "image = \"fixtures/one\"\n");
    service(&root, "two", "image = \"fixtures/two\"\n");
    service(&root, "held", "image = \"fixtures/held\"\ndeploy = false\n");

    let named = |names: &[&str]| {
        let names: Vec<String> = names.iter().map(|name| name.to_string()).collect();
        Workspace::read(&root.join("one"), &names, false)
    };
    assert_eq!(named(&["two"]).unwrap().services, [root.join("two")]);
    let mut both = named(&["held", "two"]).unwrap().services;
    both.sort();
    assert_eq!(both, [root.join("held"), root.join("two")]);
    assert!(named(&["three"]).is_err());
}

#[test]
fn directory_groups_select_only_descendant_services() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    write(
        &root,
        "Cargo.toml",
        r#"[workspace]
members = ["connect/one", "connect/nested/deep", "connect/held", "connect/library", "connect-extra/other", "utilities/watch"]
resolver = "3"
"#,
    );
    service(&root.join("connect"), "one", "image = \"fixtures/one\"\n");
    service(
        &root.join("connect/nested"),
        "deep",
        "image = \"fixtures/deep\"\n",
    );
    service(
        &root.join("connect"),
        "held",
        "image = \"fixtures/held\"\ndeploy = false\n",
    );
    service(&root.join("connect"), "library", "");
    service(
        &root.join("connect-extra"),
        "other",
        "image = \"fixtures/other\"\n",
    );
    service(
        &root.join("utilities"),
        "watch",
        "image = \"fixtures/watch\"\n",
    );

    let group = root.join("connect");
    let workspace = Workspace::read(&group.join("../connect"), &[], false).unwrap();
    let mut selected = workspace.services;
    selected.sort();
    assert_eq!(selected, [group.join("nested/deep"), group.join("one")]);
    assert_eq!(workspace.skipped, ["held"]);
    assert_eq!(workspace.root, root);

    let publishing = Workspace::read(&group, &[], true).unwrap();
    let mut selected = publishing.services;
    selected.sort();
    assert_eq!(
        selected,
        [
            group.join("held"),
            group.join("nested/deep"),
            group.join("one")
        ]
    );
    assert!(publishing.skipped.is_empty());

    let named = Workspace::read(&group, &["watch".to_owned()], false).unwrap();
    assert_eq!(named.services, [root.join("utilities/watch")]);
}

#[test]
fn directory_selection_does_not_read_unrelated_service_manifests() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    write(
        &root,
        "Cargo.toml",
        "[workspace]\nmembers = [\"connect/*\", \"outside\"]\nresolver = \"3\"\n",
    );
    service(&root.join("connect"), "one", "image = \"fixtures/one\"\n");
    service(&root, "outside", "invalid toml");

    let group = root.join("connect");
    let workspace = Workspace::read(&group, &[], true).unwrap();
    assert_eq!(workspace.services, [group.join("one")]);

    let empty = root.join("empty");
    fs::create_dir(&empty).unwrap();
    let workspace = Workspace::read(&empty, &[], true).unwrap();
    assert!(workspace.services.is_empty());
    assert!(workspace.skipped.is_empty());
}
