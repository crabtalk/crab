//! What `cargo metadata` reports of a workspace's own packages.

use anyhow::{Context, Result, bail};
use serde::Deserialize;
use std::{
    path::{Path, PathBuf},
    process::Command,
};

/// `cargo metadata --no-deps`.
#[derive(Deserialize)]
pub struct Metadata {
    pub packages: Vec<Package>,
    pub target_directory: PathBuf,
}

#[derive(Deserialize)]
pub struct Package {
    pub name: String,
    pub version: String,
    pub manifest_path: PathBuf,
    pub description: Option<String>,
    pub homepage: Option<String>,
    pub repository: Option<String>,
    pub targets: Vec<Target>,
}

#[derive(Deserialize)]
pub struct Target {
    pub crate_types: Vec<String>,
}

impl Metadata {
    /// The workspace `dir` is in, as `cargo` reports it.
    pub fn read(cargo: &str, dir: &Path) -> Result<Self> {
        let output = Command::new(cargo)
            .args(["metadata", "--format-version", "1", "--no-deps"])
            .current_dir(dir)
            .output()
            .context("cannot run cargo")?;
        if !output.status.success() {
            bail!("{}", String::from_utf8_lossy(&output.stderr).trim());
        }
        serde_json::from_slice(&output.stdout).context("cargo metadata answered what it never does")
    }

    /// The package named `name`, or, without one, the package whose manifest
    /// is `dir/Cargo.toml`.
    pub fn package(&self, dir: &Path, name: Option<&str>) -> Result<&Package> {
        if let Some(name) = name {
            return self
                .packages
                .iter()
                .find(|package| package.name == name)
                .with_context(|| format!("no package named {name} in the workspace"));
        }
        let manifest = dir
            .join("Cargo.toml")
            .canonicalize()
            .with_context(|| format!("no Cargo.toml in {}", dir.display()))?;
        self.packages
            .iter()
            .find(|package| package.manifest_path == manifest)
            .with_context(|| format!("{} is not a package", manifest.display()))
    }
}

impl Package {
    /// The directory of its `Cargo.toml`.
    pub fn dir(&self) -> &Path {
        self.manifest_path.parent().unwrap_or(Path::new("."))
    }
}
