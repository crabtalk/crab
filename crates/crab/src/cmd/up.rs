use crate::{cmd::dev, deployment, project, runtime};
use anyhow::{Context, Result, bail};
use clap::Args;
use crabtalk_sdk::{
    client::Client,
    services::service::{DeployRequest, UploadRequest},
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

#[derive(Args)]
pub struct Up {
    #[arg(default_value = ".")]
    pub dir: PathBuf,
    #[arg(short = 'p', long = "package")]
    pub packages: Vec<String>,
    #[arg(long)]
    pub force: bool,
    #[arg(long)]
    pub build: bool,
    #[arg(long = "copy", value_name = "SERVICE:KEY=VALUE")]
    pub copy: Vec<String>,
}
impl Up {
    pub fn run(&self, client: &Client, yes: bool) -> Result<()> {
        let copy_values = self.copies()?;
        if (yes || client.owner.is_none()) && !copy_values.is_empty() {
            bail!("--copy supplies values for the Cloud deploy page; deploy without --yes");
        }
        let selection = project::Selection::read(&self.dir, &self.packages, true)?;
        if selection.services.is_empty() {
            bail!("no service at or below {}", self.dir.display());
        }
        if client.base == dev::url("") {
            dev::up()?;
        }
        let mut names = BTreeSet::new();
        let mut services = BTreeMap::new();
        for service in selection.services {
            let name = service.name()?.to_owned();
            if !names.insert(name.clone()) {
                bail!("more than one selected service deploys as {name}");
            }
            let built = berm_build::build(&service.dir, None)?;
            let bytes =
                fs::read(&built).with_context(|| format!("cannot read {}", built.display()))?;
            let reference = deployment::send(client, &name, bytes)?;
            let seed = match &service.project.database.seed {
                Some(_) if client.owner.is_none() => {
                    bail!("{name}: a seed starts a service on Cloud")
                }
                Some(seed) if runtime::wait(first(client, &name))? => {
                    Some(upload(client, &name, &service.dir.join(seed))?)
                }
                _ => None,
            };
            services.insert(
                name,
                DeployRequest {
                    reference,
                    size: Some(service.project.database.size()),
                    seed,
                    force: self.force,
                    project: selection.project.clone(),
                    ..Default::default()
                },
            );
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
