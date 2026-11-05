//! The services of a cargo workspace.

use crate::{metadata::Metadata, toolchain::Toolchain};
use anyhow::{Context, Result, bail};
use berm_api::project::{self, Project};
use std::path::{Path, PathBuf};

/// A workspace and the services in it a command acts on.
pub struct Workspace {
    pub root: PathBuf,
    /// cargo's target directory.
    pub target: PathBuf,
    /// The directory of each service's `Cargo.toml`.
    pub services: Vec<PathBuf>,
    /// The names of the services left out for `deploy = false`.
    pub skipped: Vec<String>,
}

impl Workspace {
    /// Explicit names select across the Cargo workspace. Otherwise, select
    /// the service in `dir`, or services beneath it. Directory groups omit
    /// `deploy = false` unless `include_disabled` is set.
    pub fn read(dir: &Path, named: &[String], include_disabled: bool) -> Result<Self> {
        let toolchain = Toolchain::new(dir)?;
        let metadata = Metadata::read(&toolchain, dir)?;
        let mut services = Vec::new();
        let mut skipped = Vec::new();
        let here = dir
            .canonicalize()
            .with_context(|| format!("cannot resolve {}", dir.display()))?;
        let mut found = Vec::new();
        for package in &metadata.packages {
            let dir = package.manifest_path.dir();
            let package_dir = dir
                .canonicalize()
                .with_context(|| format!("cannot resolve {}", dir.display()))?;
            if named.is_empty() && !package_dir.starts_with(&here) {
                continue;
            }
            let path = dir.join(project::FILE);
            if !path.is_file() {
                continue;
            }
            let project = Project::read(&path)?;
            let Some(name) = project.name else {
                continue;
            };
            if !named.is_empty() {
                if named.contains(&name) {
                    services.push(dir.to_path_buf());
                    found.push(name);
                }
                continue;
            }
            if here == package_dir {
                return Ok(Self {
                    root: metadata.workspace_root.clone(),
                    target: metadata.target_directory.clone(),
                    services: vec![dir.to_path_buf()],
                    skipped: Vec::new(),
                });
            }
            match project.deploy || include_disabled {
                true => services.push(dir.to_path_buf()),
                false => skipped.push(name),
            }
        }
        if let Some(missing) = named.iter().find(|name| !found.contains(name)) {
            bail!(
                "no service named {missing} in the workspace at {}",
                metadata.workspace_root.display()
            );
        }
        Ok(Self {
            root: metadata.workspace_root,
            target: metadata.target_directory,
            services,
            skipped,
        })
    }
}
