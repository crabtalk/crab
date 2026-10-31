//! Which crates the build takes for a service's kits: each it compiled for the
//! guest in the service's own build, with a `crab.toml`.

use berm_abi::ABI_VERSION;
use berm_api::Manifest;
use std::{fs, path::Path};
use tempfile::TempDir;

/// Write `text` to `path` under `root`, creating its directories.
fn write(root: &Path, path: &str, text: &str) {
    let path = root.join(path);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

/// A crate `name` at `version` in `dir`, with `dependencies` as written.
fn package(root: &Path, dir: &str, name: &str, version: &str, dependencies: &str) {
    write(
        root,
        &format!("{dir}/Cargo.toml"),
        &format!(
            "[package]\nname = \"{name}\"\nversion = \"{version}\"\nedition = \"2024\"\n\n[dependencies]\n{dependencies}\n"
        ),
    );
    write(root, &format!("{dir}/src/lib.rs"), "#![no_std]\n");
}

/// The service in `root/service`, a cdylib depending on what `dependencies`
/// and `dev` name.
fn service(root: &Path, dependencies: &str, dev: &str) {
    write(
        root,
        "service/Cargo.toml",
        &format!(
            "[package]\nname = \"service\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[workspace]\n\n[lib]\ncrate-type = [\"cdylib\", \"rlib\"]\n\n[dependencies]\n{dependencies}\n\n[dev-dependencies]\n{dev}\n"
        ),
    );
    write(root, "service/src/lib.rs", "");
    write(root, "service/crab.toml", "name = \"service\"\n");
}

/// A `crab.toml` a kit may not have, so taking the crate for a kit fails the
/// build naming it.
const NOT_A_KIT: &str = "id = \"invalid\"\n";

#[test]
fn a_kit_at_two_versions_is_refused() {
    let root = TempDir::new().unwrap();
    let root = root.path();
    service(
        root,
        "kit = { path = \"../kit1\" }\nother = { path = \"../other\" }",
        "",
    );
    package(root, "kit1", "kit", "0.1.0", "");
    write(root, "kit1/crab.toml", "secrets = [\"one\"]\n");
    package(root, "kit2", "kit", "0.2.0", "");
    write(root, "kit2/crab.toml", "secrets = [\"two\"]\n");
    package(
        root,
        "other",
        "other",
        "0.1.0",
        "kit = { path = \"../kit2\" }",
    );

    let error = berm_build::build(&root.join("service"), None).unwrap_err();
    assert!(
        error.to_string().contains("kit kit at two versions"),
        "{error:#}"
    );
}

#[test]
fn a_kit_behind_another_crate_is_found() {
    let root = TempDir::new().unwrap();
    let root = root.path();
    service(root, "facade = { path = \"../facade\" }", "");
    package(
        root,
        "facade",
        "facade",
        "0.1.0",
        "kit = { path = \"../kit\" }",
    );
    package(root, "kit", "kit", "0.1.0", "");
    write(root, "kit/crab.toml", NOT_A_KIT);

    let error = berm_build::build(&root.join("service"), None).unwrap_err();
    assert!(format!("{error:#}").contains("the kit kit"), "{error:#}");
}

#[test]
fn a_dev_dependency_is_no_kit() {
    let root = TempDir::new().unwrap();
    let root = root.path();
    service(root, "", "kit = { path = \"../kit\" }");
    package(root, "kit", "kit", "0.1.0", "");
    write(root, "kit/crab.toml", NOT_A_KIT);

    // Built past its kits, it is refused only for what it lacks itself.
    let error = berm_build::build(&root.join("service"), None).unwrap_err();
    assert!(format!("{error:#}").contains("no tools"), "{error:#}");
}

#[test]
fn a_service_without_a_cdylib_is_refused() {
    let root = TempDir::new().unwrap();
    let root = root.path();
    package(root, "service", "service", "0.1.0", "");
    write(root, "service/crab.toml", "name = \"service\"\n");
    // Its own workspace, apart from the one the test runs in.
    let manifest = root.join("service/Cargo.toml");
    let text = fs::read_to_string(&manifest).unwrap();
    fs::write(&manifest, text + "\n[workspace]\n").unwrap();

    let error = berm_build::build(&root.join("service"), None).unwrap_err();
    assert!(error.to_string().contains("builds no cdylib"), "{error:#}");
}

#[test]
fn a_service_naming_no_name_is_refused() {
    let root = TempDir::new().unwrap();
    let root = root.path();
    service(root, "", "");
    write(root, "service/crab.toml", "deploy = true\n");

    let error = berm_build::build(&root.join("service"), None).unwrap_err();
    assert!(error.to_string().contains("names itself"), "{error:#}");
}

/// A service `name` in `root/name` whose only tool, `run`, is a record and an
/// export written by hand, depending on what `dependencies` names.
fn tooled(root: &Path, name: &str, dependencies: &str) {
    write(
        root,
        &format!("{name}/Cargo.toml"),
        &format!(
            "[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[lib]\ncrate-type = [\"cdylib\", \"rlib\"]\n\n[dependencies]\n{dependencies}\n"
        ),
    );
    let record = format!(
        r#"{{"tool":{{"name":"run","description":"d","parameters":{{"type":"object"}}}},"abi_version":{ABI_VERSION},"crate":"{name}"}}"#
    );
    write(
        root,
        &format!("{name}/src/lib.rs"),
        &format!(
            "#[used]\n#[unsafe(link_section = \".berm.abi\")]\nstatic RECORD: [u8; {}] = *br#\"{record}\"#;\n\n#[unsafe(no_mangle)]\npub extern \"C\" fn berm_tool_run() {{}}\n",
            record.len()
        ),
    );
    write(
        root,
        &format!("{name}/crab.toml"),
        &format!("name = \"{name}\"\n"),
    );
}

/// The kits the finished image of the service in `dir` names.
fn kits(dir: &Path) -> Vec<String> {
    let image = berm_build::build(dir, None).unwrap();
    let manifest = Manifest::try_from(fs::read(image).unwrap().as_slice()).unwrap();
    manifest.kits.into_iter().map(|kit| kit.name).collect()
}

#[test]
fn each_service_is_credited_only_the_kits_its_own_features_link() {
    let root = TempDir::new().unwrap();
    let root = root.path();
    write(
        root,
        "Cargo.toml",
        "[workspace]\nmembers = [\"a\", \"b\", \"facade\", \"kita\", \"kitb\"]\nresolver = \"3\"\n",
    );
    tooled(
        root,
        "a",
        "facade = { path = \"../facade\", features = [\"a\"] }",
    );
    tooled(
        root,
        "b",
        "facade = { path = \"../facade\", features = [\"b\"] }",
    );
    package(
        root,
        "facade",
        "facade",
        "0.1.0",
        "kita = { path = \"../kita\", optional = true }\nkitb = { path = \"../kitb\", optional = true }\n\n[features]\na = [\"dep:kita\"]\nb = [\"dep:kitb\"]",
    );
    package(root, "kita", "kita", "0.1.0", "");
    write(root, "kita/crab.toml", "secrets = [\"a\"]\n");
    package(root, "kitb", "kitb", "0.1.0", "");
    write(root, "kitb/crab.toml", "secrets = [\"b\"]\n");

    assert_eq!(kits(&root.join("a")), ["kita"]);
    assert_eq!(kits(&root.join("b")), ["kitb"]);
    // Built again with nothing changed, every crate is fresh.
    assert_eq!(kits(&root.join("a")), ["kita"]);
}
