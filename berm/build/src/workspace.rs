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
}

impl Workspace {
    /// The workspace `dir` is in, with the services `dir` names as cargo names
    /// packages: the package whose `Cargo.toml` is in `dir` when its
    /// `berm.toml` names a service, and otherwise every member whose
    /// `berm.toml` does.
    pub fn read(dir: &Path) -> Result<Self> {
        let cargo = env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned());
        let metadata = Metadata::read(&cargo, dir)?;
        let mut services = Vec::new();
        for package in &metadata.packages {
            let dir = package.dir();
            if dir.join(Project::FILE).is_file() && Project::read(dir)?.name.is_some() {
                services.push(dir.to_path_buf());
            }
        }
        if let Ok(here) = dir.canonicalize()
            && services.contains(&here)
        {
            services = vec![here];
        }
        Ok(Self {
            root: metadata.workspace_root,
            target: metadata.target_directory,
            services,
        })
    }
}
