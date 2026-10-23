//! Which of a workspace's services a command acts on: every one but those
//! whose `crab.toml` says `deploy = false`, or the one whose directory it was
//! run in.

use berm_build::Workspace;
use std::{fs, path::Path};

/// What the programs and kits here call their project file.
const FILE: &str = "crab.toml";

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
    service(&root, "shown", "name = \"shown\"\n");
    service(&root, "held", "name = \"held\"\ndeploy = false\n");

    let workspace = Workspace::read(&root, FILE).unwrap();
    assert_eq!(workspace.services, [root.join("shown")]);
    assert_eq!(workspace.skipped, ["held"]);

    let named = Workspace::read(&root.join("held"), FILE).unwrap();
    assert_eq!(named.services, [root.join("held")]);
    assert!(named.skipped.is_empty());
}
