use crate::{cmd::dev, deployment, project, runtime};
use anyhow::{Context, Result, bail};
use berm_api::reference::Reference as ImageReference;
use clap::Args;
use crabtalk_sdk::{
    client::Client,
    services::service::{DeployRequest, TemplateSource, UploadRequest},
};
use rusqlite::{Connection, OpenFlags};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File},
    io::Read,
    path::{Path, PathBuf},
};
use tempfile::TempDir;
use tokio::fs::File as AsyncFile;

/// Service selection and image source.
#[derive(Args)]
pub struct Deploy {
    /// Service directory or parent directory containing services.
    #[arg(default_value = ".")]
    pub dir: PathBuf,
    /// Select this service by name across the workspace; repeatable.
    #[arg(short = 'p', long = "package")]
    pub packages: Vec<String>,
    /// Push images to this registry, overriding the context, as `localhost:5001`.
    #[arg(long)]
    pub registry: Option<String>,
    /// Deploy even when it breaks what callers used.
    #[arg(long)]
    pub force: bool,
    /// Build local source instead of deploying its published image.
    #[arg(long, conflicts_with_all = ["from", "version", "auto_upgrade"])]
    pub build: bool,
    /// Deploy a published template, `owner/name`, as a new private service on Cloud.
    #[arg(long, value_name = "OWNER/NAME", conflicts_with_all = ["packages", "registry"])]
    pub from: Option<String>,
    /// Select an exact retained template version.
    #[arg(long)]
    pub version: Option<String>,
    /// Automatically install stable patches in the same major and minor version.
    #[arg(long)]
    pub auto_upgrade: bool,
    /// Provider value for the deploy page, SERVICE:KEY=VALUE; repeatable.
    #[arg(long = "copy", value_name = "SERVICE:KEY=VALUE")]
    pub copy: Vec<String>,
}

impl Deploy {
    pub fn run(&self, client: &Client, yes: bool, context_registry: Option<&str>) -> Result<()> {
        let copy_values = self.copies()?;
        if (yes || client.owner.is_none()) && !copy_values.is_empty() {
            bail!("--copy supplies values for the Cloud deploy page; deploy without --yes");
        }
        // An explicit remote source does not require a directory or Cargo project.
        if let Some(from) = &self.from {
            let (name, request) = self.template_request(client, from, self.version.as_deref())?;
            return deployment::submit(client, BTreeMap::from([(name, request)]), copy_values, yes);
        }
        let selection = project::Selection::read(&self.dir, &self.packages, self.build, false)?;
        if selection.services.is_empty() {
            bail!("no service at or below {}", self.dir.display());
        }
        for name in &selection.skipped {
            eprintln!("skipped   {name}: deploy = false");
        }
        if client.base == dev::url("")
            && selection
                .services
                .iter()
                .any(|service| self.build || service.project.image.is_none())
        {
            dev::up()?;
        }
        let mut names = BTreeSet::new();
        for service in &selection.services {
            let name = service.name()?;
            if !names.insert(name) {
                bail!("more than one selected service deploys as {name}");
            }
            service.project.check_deploy()?;
        }
        let mut services = BTreeMap::new();
        for service in selection.services {
            let name = service.name()?.to_owned();
            let mut request = if !self.build
                && let Some(image) = &service.project.image
            {
                let from = image.address();
                let version = match &self.version {
                    Some(version) => version.clone(),
                    None => service.version()?,
                };
                self.template_request(client, &from, Some(&version))?.1
            } else {
                if self.version.is_some() || self.auto_upgrade {
                    bail!("--version and --auto-upgrade require a published image");
                }
                let built = berm_build::build(&service.dir, None)?;
                let bytes =
                    fs::read(&built).with_context(|| format!("cannot read {}", built.display()))?;
                let reference = deployment::reference(
                    client,
                    &name,
                    &service
                        .project
                        .image
                        .as_ref()
                        .map_or_else(|| name.clone(), |image| image.address()),
                    self.registry.as_deref().or(context_registry),
                    bytes,
                )?;
                let seed = match &service.project.database.seed {
                    Some(_) if client.owner.is_none() => bail!(
                        "{name}: a seed starts a service on Cloud; the local host's own database is where one is made"
                    ),
                    Some(seed) if runtime::wait(first(client, &name))? => {
                        Some(upload(client, &name, &service.dir.join(seed))?)
                    }
                    _ => None,
                };
                DeployRequest {
                    reference,
                    public: Some(service.project.public),
                    size: Some(service.project.database.size()),
                    seed,
                    force: self.force,
                    prices: project::prices(&service.project)?,
                    ..DeployRequest::default()
                }
            };
            request.project = selection.project.clone();
            if services.insert(name.clone(), request).is_some() {
                bail!("more than one selected service deploys as {name}");
            }
        }
        deployment::submit(client, services, copy_values, yes)
    }

    fn copies(&self) -> Result<BTreeMap<String, BTreeMap<String, String>>> {
        let mut copy = BTreeMap::<String, BTreeMap<String, String>>::new();
        for value in &self.copy {
            let (key, value) = value
                .split_once('=')
                .context("--copy takes SERVICE:KEY=VALUE")?;
            let (service, key) = key
                .split_once(':')
                .context("--copy takes SERVICE:KEY=VALUE")?;
            if service.is_empty() || key.is_empty() {
                bail!("--copy names a service and a key");
            }
            if copy
                .entry(service.to_owned())
                .or_default()
                .insert(key.to_owned(), value.to_owned())
                .is_some()
            {
                bail!("--copy repeats {service}:{key}");
            }
        }
        Ok(copy)
    }

    fn template_request(
        &self,
        client: &Client,
        from: &str,
        version: Option<&str>,
    ) -> Result<(String, DeployRequest)> {
        if client.owner.is_none() {
            bail!("published images require a Cloud context; use --build with local source");
        }
        if self.registry.is_some() {
            bail!("--registry requires --build when image is declared");
        }
        let image: ImageReference = from.parse()?;
        let address = image.address();
        let pinned = image.version.as_ref().map(ToString::to_string);
        let version = version.or(pinned.as_deref());
        let template = runtime::wait(async {
            match version {
                Some(version) => client.template_version(&address, version).await,
                None => client.template(&address).await,
            }
        })?;
        Ok((
            image.name,
            DeployRequest {
                public: Some(false),
                template: Some(false),
                source: Some(TemplateSource {
                    owner: template.owner,
                    name: template.name,
                }),
                auto_upgrade: Some(self.auto_upgrade),
                create: true,
                reference: template.digest,
                force: self.force,
                ..DeployRequest::default()
            },
        ))
    }
}

/// Whether `name` is not deployed yet.
async fn first(client: &Client, name: &str) -> Result<bool> {
    match client.runtime(name).await {
        Ok(_) => Ok(false),
        Err(crabtalk_sdk::client::Error::Api { status: 404, .. }) => Ok(true),
        Err(error) => Err(error.into()),
    }
}

/// Upload a copy of the database at `seed` for `name`'s deploy unless Cloud
/// holds it already, answering its SHA-256.
fn upload(client: &Client, name: &str, seed: &Path) -> Result<String> {
    let dir = TempDir::new().context("creating a directory for the seed's copy")?;
    let copy = dir.path().join("seed.sqlite");
    let Some(into) = copy.to_str() else {
        bail!("the seed's copy {} is not a UTF-8 path", copy.display());
    };
    // One consistent file, its write-ahead log folded in, while the local host may
    // hold the database open.
    Connection::open_with_flags(seed, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .with_context(|| format!("opening the seed {}", seed.display()))?
        .execute("VACUUM INTO ?1", [into])
        .with_context(|| format!("copying the seed {}", seed.display()))?;
    let (sha256, size) = hash(&copy)?;
    let request = UploadRequest {
        sha256: sha256.clone(),
        size,
    };
    if let Some(url) = runtime::wait(client.seed(name, &request))?.upload {
        eprintln!("uploading the seed, {size} bytes");
        let file = runtime::wait(AsyncFile::open(&copy))?;
        runtime::wait(client.upload(&url, file, size))?;
    }
    Ok(sha256)
}

/// The SHA-256 of the file at `path`, in hex, and its size.
fn hash(path: &Path) -> Result<(String, u64)> {
    let mut file = File::open(path)?;
    let (mut hasher, mut size, mut buffer) = (Sha256::new(), 0, vec![0; 1 << 20]);
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            return Ok((hex::encode(hasher.finalize()), size));
        }
        hasher.update(&buffer[..read]);
        size += read as u64;
    }
}
