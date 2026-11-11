//! A service's or kit's `crab.toml`: reading it, and writing it with its
//! layout kept as written.

use anyhow::{Context, Result, anyhow, bail};
use berm_api::project::Project;
use berm_build::Workspace;
use cargo_toml::{Manifest, Package};
use crabtalk_sdk::{catalog::Catalog, services::price::Price};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

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
    let selected = Selection::read(Path::new("."), &[], false, false)?;
    let names = selected
        .services
        .iter()
        .map(|service| service.name().map(str::to_owned))
        .collect::<Result<Vec<_>>>()?;
    if names.is_empty() {
        bail!("no service here; name one");
    }
    Ok(names)
}

pub struct Service {
    pub dir: PathBuf,
    pub project: Project,
    pub package: Option<Package>,
}

impl Service {
    pub fn read(dir: &Path) -> Result<Self> {
        let project = read(dir)?;
        project.check_service()?;
        let manifest = dir.join("Cargo.toml");
        let package = if manifest.is_file() {
            Manifest::from_path(&manifest)
                .with_context(|| format!("cannot read {}", manifest.display()))?
                .package
        } else {
            None
        };
        Ok(Self {
            dir: dir.to_path_buf(),
            project,
            package,
        })
    }

    pub fn name(&self) -> Result<&str> {
        let package = self
            .package
            .as_ref()
            .map_or("", |package| package.name.as_str());
        let name = self.project.instance(package);
        if name.is_empty() {
            bail!(
                "{} has neither an image address nor a Cargo package",
                self.dir.display()
            );
        }
        Ok(name)
    }

    pub fn version(&self) -> Result<String> {
        if let Some(version) = self
            .project
            .image
            .as_ref()
            .and_then(|image| image.version.as_ref())
        {
            return Ok(version.to_string());
        }
        self.package
            .as_ref()
            .map(|package| package.version().to_string())
            .with_context(|| {
                format!(
                    "{}: image needs an exact :version without Cargo.toml",
                    self.dir.display()
                )
            })
    }

    pub fn source_version(&self) -> Result<String> {
        let package = self
            .package
            .as_ref()
            .context("a source build needs Cargo.toml")?;
        let version = package.version().to_string();
        if let Some(pinned) = self
            .project
            .image
            .as_ref()
            .and_then(|image| image.version.as_ref())
            && pinned.to_string() != version
        {
            bail!("image version {pinned} differs from Cargo package version {version}");
        }
        Ok(version)
    }
}

pub struct Selection {
    pub project: Option<String>,
    pub services: Vec<Service>,
    pub skipped: Vec<String>,
}

impl Selection {
    pub fn read(
        dir: &Path,
        named: &[String],
        building: bool,
        include_disabled: bool,
    ) -> Result<Self> {
        if named.is_empty() && dir.join(FILE).is_file() {
            let service = Service::read(dir)?;
            if !building && service.project.image.is_some() {
                return Ok(Self {
                    project: None,
                    services: vec![service],
                    skipped: Vec::new(),
                });
            }
            if service.package.is_none() {
                bail!("a source build needs Cargo.toml in {}", dir.display());
            }
        }
        let workspace = Workspace::read(dir, named, include_disabled)?;
        let project = workspace
            .root
            .file_name()
            .and_then(|name| name.to_str())
            .with_context(|| format!("{} names no project", workspace.root.display()))?
            .to_owned();
        let services = workspace
            .services
            .iter()
            .map(|dir| Service::read(dir))
            .collect::<Result<Vec<_>>>()?;
        Ok(Self {
            project: Some(project),
            services,
            skipped: workspace.skipped,
        })
    }
}
