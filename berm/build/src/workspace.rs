//! The services of a cargo workspace.

use crate::metadata::Metadata;
use anyhow::{Result, bail};
use berm_api::project::Project;
use std::{
    env,
    path::{Path, PathBuf},
};

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
    /// The workspace `dir` is in, with the services a command names as cargo
    /// names packages: those `named` by service name, anywhere in the
    /// workspace; else the package whose `Cargo.toml` is in `dir` when its
    /// project file, `file`, names a service; else every member whose does and
    /// does not say `deploy = false`.
    pub fn read(dir: &Path, file: &str, named: &[String]) -> Result<Self> {
        let cargo = env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned());
        let metadata = Metadata::read(&cargo, dir)?;
        let mut services = Vec::new();
        let mut skipped = Vec::new();
        let here = dir.canonicalize().ok();
        let mut found = Vec::new();
        for package in &metadata.packages {
            let dir = package.dir();
            let path = dir.join(file);
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
            if here.as_deref() == Some(dir) {
                return Ok(Self {
                    root: metadata.workspace_root.clone(),
                    target: metadata.target_directory.clone(),
                    services: vec![dir.to_path_buf()],
                    skipped: Vec::new(),
                });
            }
            match project.deploy {
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
