//! A service's or kit's `crab.toml`: reading it, and writing it with its
//! layout kept as written.

use anyhow::{Context, Result, anyhow, bail};
use berm_api::project::Project;
use berm_build::Workspace;
use crabtalk_sdk::{catalog::Catalog, services::price::Price};
use std::{collections::BTreeMap, path::Path};

/// What the project file beside a service's or kit's `Cargo.toml` is called.
pub const FILE: &str = berm_api::project::FILE;

/// The `crab.toml` in `dir`, refused when a tool's price is not one or its
/// catalog job does not start with a platform the catalog lists.
pub fn read(dir: &Path) -> Result<Project> {
    let path = dir.join(FILE);
    let project = Project::read(&path)?;
    for (name, tool) in &project.tools {
        if let Some(job) = &tool.catalog {
            Catalog::listed()
                .check(job)
                .map_err(|e| anyhow!("{}: tools.{name}: {e}", path.display()))?;
        }
    }
    prices(&project).with_context(|| path.display().to_string())?;
    Ok(project)
}

/// Each price `[tools]` in `project` sets, by tool, refused naming the tool of
/// one that is not a price.
pub fn prices(project: &Project) -> Result<BTreeMap<String, Price>> {
    project
        .tools
        .iter()
        .filter_map(|(name, tool)| Some((name, tool.price?)))
        .map(|(name, micros)| {
            let price = Price::try_from(micros).map_err(|e| anyhow!("tools.{name}: {e}"))?;
            Ok((name.clone(), price))
        })
        .collect()
}

/// Explicit names, or the current directory's services selected as by
/// `crab deploy`. Fails when none are selected.
pub fn selected(names: &[String]) -> Result<Vec<String>> {
    if !names.is_empty() {
        return Ok(names.to_vec());
    }
    let workspace = Workspace::read(Path::new("."), &[], false)?;
    let mut names = Vec::new();
    for dir in &workspace.services {
        names.push(read(dir)?.name()?.to_owned());
    }
    if names.is_empty() {
        bail!("no service here; name one");
    }
    Ok(names)
}
