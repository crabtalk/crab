//! What `cargo metadata` reports of a workspace's own packages.

use anyhow::{Context, Result, bail};
use serde::Deserialize;
use std::{
    ops::Deref,
    path::{Path, PathBuf},
    process::Command,
};

/// `cargo metadata --no-deps`.
#[derive(Deserialize)]
pub struct Metadata {
    pub packages: Vec<Package>,
    pub target_directory: PathBuf,
    pub workspace_root: PathBuf,
}

#[derive(Deserialize)]
pub struct Package {
    pub name: String,
    pub version: String,
    pub manifest_path: ManifestPath,
    pub description: Option<String>,
    pub homepage: Option<String>,
    pub repository: Option<String>,
    /// Relative to the package's directory; Cargo reports `README.md` there
    /// when the manifest names none.
    pub readme: Option<PathBuf>,
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
            .find(|package| *package.manifest_path == *manifest)
            .with_context(|| format!("{} is not a package", manifest.display()))
    }
}

/// The path of a package's `Cargo.toml`, as cargo reports it.
#[derive(Debug, PartialEq, Eq, Deserialize)]
#[serde(transparent)]
pub struct ManifestPath(pub PathBuf);

impl ManifestPath {
    /// The directory of the `Cargo.toml`.
    pub fn dir(&self) -> &Path {
        self.0.parent().unwrap_or(Path::new("."))
    }
}

impl Deref for ManifestPath {
    type Target = Path;

    fn deref(&self) -> &Path {
        &self.0
    }
}
