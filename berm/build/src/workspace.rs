//! The services of a cargo workspace.

use crate::metadata::Metadata;
use anyhow::Result;
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
    /// The workspace `dir` is in, with the services `dir` names as cargo names
    /// packages: the package whose `Cargo.toml` is in `dir` when its project
    /// file, `file`, names a service, and otherwise every member whose does
    /// and does not say `deploy = false`.
    pub fn read(dir: &Path, file: &str) -> Result<Self> {
        let cargo = env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned());
        let metadata = Metadata::read(&cargo, dir)?;
        let mut services = Vec::new();
        let mut skipped = Vec::new();
        let here = dir.canonicalize().ok();
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
        Ok(Self {
            root: metadata.workspace_root,
            target: metadata.target_directory,
            services,
            skipped,
        })
    }
}
