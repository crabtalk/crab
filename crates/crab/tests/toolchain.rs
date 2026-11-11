#![cfg(unix)]

use std::{
    fs,
    os::unix::fs::{self as unix_fs, PermissionsExt},
    path::Path,
    process::{Command, Output},
};

struct Fixture {
    root: tempfile::TempDir,
}

impl Fixture {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        for name in ["bin", "tools", "project", "sysroot"] {
            fs::create_dir(root.path().join(name)).unwrap();
        }
        unix_fs::symlink("/bin/sh", root.path().join("bin/sh")).unwrap();
        fs::write(root.path().join("project/crab.toml"), "").unwrap();
        fs::write(
            root.path().join("project/Cargo.toml"),
            "[package]\nname = \"example\"\nversion = \"0.1.0\"\n",
        )
        .unwrap();
        Self::executable(
            &root.path().join("tools/cargo"),
            r#"#!/bin/sh
if [ "$1" = --version ]; then echo cargo; exit 0; fi
echo metadata >> "$CRAB_TOOLCHAIN_TEST/log"
echo 'metadata reached' >&2
exit 42
"#,
        );
        Self::executable(
            &root.path().join("tools/rustc"),
            r#"#!/bin/sh
if [ "$1" = --version ]; then echo rustc; else echo "$CRAB_TOOLCHAIN_TEST/sysroot"; fi
"#,
        );
        Self::executable(
            &root.path().join("tools/rustup"),
            r#"#!/bin/sh
case "$1" in
  --version) echo rustup ;;
  default)
    if [ "$2" = stable ]; then
      echo default >> "$CRAB_TOOLCHAIN_TEST/log"
      /bin/rm -f "$CRAB_TOOLCHAIN_TEST/no-default"
    elif [ -f "$CRAB_TOOLCHAIN_TEST/no-default" ]; then exit 1; fi ;;
  toolchain) echo toolchain >> "$CRAB_TOOLCHAIN_TEST/log" ;;
  target)
    echo target >> "$CRAB_TOOLCHAIN_TEST/log"
    /bin/mkdir -p "$CRAB_TOOLCHAIN_TEST/sysroot/lib/rustlib/wasm32-unknown-unknown" ;;
esac
"#,
        );
        Self::executable(
            &root.path().join("bin/curl"),
            r#"#!/bin/sh
echo download >> "$CRAB_TOOLCHAIN_TEST/log"
while [ "$#" -gt 0 ]; do
  if [ "$1" = --output ]; then /bin/cp "$CRAB_TOOLCHAIN_TEST/installer" "$2"; exit; fi
  shift
done
exit 1
"#,
        );
        fs::write(root.path().join("installer"), r#"echo "install $*" >> "$CRAB_TOOLCHAIN_TEST/log"
if [ "$FAIL_INSTALL" = 1 ]; then exit 1; fi
/bin/mkdir -p "$CARGO_HOME/bin"
/bin/cp "$CRAB_TOOLCHAIN_TEST/tools/cargo" "$CRAB_TOOLCHAIN_TEST/tools/rustc" "$CRAB_TOOLCHAIN_TEST/tools/rustup" "$CARGO_HOME/bin/"
"#).unwrap();
        Self { root }
    }

    fn executable(path: &Path, script: &str) {
        fs::write(path, script).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
    }

    fn installed(&self) {
        for tool in ["cargo", "rustc", "rustup"] {
            fs::copy(
                self.root.path().join("tools").join(tool),
                self.root.path().join("bin").join(tool),
            )
            .unwrap();
        }
    }

    fn command(&self, operation: &str) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_crab"));
        command
            .arg(operation)
            .arg(self.root.path().join("project"))
            .env("CRAB_TOOLCHAIN_TEST", self.root.path())
            .env("CARGO_HOME", self.root.path().join("cargo-home"))
            .env("RUSTUP_HOME", self.root.path().join("rustup-home"))
            .env("PATH", self.root.path().join("bin"))
            .env_remove("CARGO")
            .env_remove("RUSTC")
            .env_remove("RUSTUP_TOOLCHAIN")
            .env_remove("RUSTC_WRAPPER")
            .env_remove("RUSTC_WORKSPACE_WRAPPER");
        command
    }

    fn reached_metadata(&self) -> Output {
        let output = self.command("build").output().unwrap();
        assert!(!output.status.success());
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("metadata reached"),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stdout.is_empty());
        output
    }

    fn log(&self) -> String {
        fs::read_to_string(self.root.path().join("log")).unwrap()
    }
}

#[test]
fn a_first_build_installs_tools_and_target_before_cargo_and_reuses_them() {
    let fixture = Fixture::new();
    fixture.reached_metadata();
    let log = fixture.log();
    assert!(log.starts_with("download\ninstall "));
    assert!(log.contains("--no-modify-path"));
    assert!(log.ends_with("target\nmetadata\n"));
    fixture.reached_metadata();
    assert!(fixture.log().ends_with("target\nmetadata\nmetadata\n"));
    assert_eq!(fixture.log().matches("download").count(), 1);
}

#[test]
fn an_existing_toolchain_only_installs_the_missing_target() {
    let fixture = Fixture::new();
    fixture.installed();
    fixture.reached_metadata();
    assert_eq!(fixture.log(), "target\nmetadata\n");
}

#[test]
fn a_ready_toolchain_does_not_install_anything() {
    let fixture = Fixture::new();
    fixture.installed();
    fs::create_dir_all(
        fixture
            .root
            .path()
            .join("sysroot/lib/rustlib/wasm32-unknown-unknown"),
    )
    .unwrap();
    fixture.reached_metadata();
    assert_eq!(fixture.log(), "metadata\n");
}

#[test]
fn installation_failure_stops_before_cargo_and_explains_the_failure() {
    let fixture = Fixture::new();
    let output = fixture
        .command("build")
        .env("FAIL_INSTALL", "1")
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("could not install the build tools"));
    assert!(!fixture.log().contains("metadata"));
}

#[test]
fn rustup_without_a_default_gets_a_toolchain_before_building() {
    let fixture = Fixture::new();
    fixture.installed();
    fs::write(fixture.root.path().join("no-default"), "").unwrap();
    Fixture::executable(
        &fixture.root.path().join("bin/cargo"),
        r#"#!/bin/sh
if [ -f "$CRAB_TOOLCHAIN_TEST/no-default" ]; then exit 1; fi
if [ "$1" = --version ]; then echo cargo; exit 0; fi
echo metadata >> "$CRAB_TOOLCHAIN_TEST/log"
echo 'metadata reached' >&2
exit 42
"#,
    );
    fixture.reached_metadata();
    assert_eq!(fixture.log(), "toolchain\ndefault\ntarget\nmetadata\n");
}

#[test]
fn an_existing_compiler_without_rustup_gets_the_target_automatically() {
    let fixture = Fixture::new();
    fixture.installed();
    fs::remove_file(fixture.root.path().join("bin/rustup")).unwrap();
    fixture.reached_metadata();
    assert!(fixture.log().starts_with("download\ninstall "));
    assert!(fixture.log().ends_with("target\nmetadata\n"));
}

#[test]
fn a_published_image_does_not_prepare_build_tools() {
    let fixture = Fixture::new();
    fs::write(
        fixture.root.path().join("project/crab.toml"),
        "image = \"connect/tinyfish:1.2.3\"\n",
    )
    .unwrap();
    let output = fixture
        .command("deploy")
        .args(["--context", "default"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("Cloud context"));
    assert!(!fixture.root.path().join("log").exists());
}

#[test]
fn build_explicitly_selects_local_source() {
    let fixture = Fixture::new();
    fs::write(
        fixture.root.path().join("project/crab.toml"),
        "image = \"connect/tinyfish\"\n",
    )
    .unwrap();
    let output = fixture
        .command("deploy")
        .args(["--build", "--context", "default"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("metadata reached"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(fixture.log().ends_with("metadata\n"));
}
