//! `crab deploy` and `crab push` — build services and hand the context's host
//! their images, serving or stopped.

use crate::{
    cmd::{dev, inspect},
    project, utils,
};
use anyhow::{Context, Result, bail};
use berm_api::Manifest;
use berm_build::Workspace;
use berm_oci::{Access, Reference, Registry};
use clap::Args;
use crabtalk_sdk::{
    client::{Client, Error},
    sandbox::service::{DeployRequest, SeedRequest},
};
use rusqlite::{Connection, OpenFlags};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File},
    io::Read,
    path::{Path, PathBuf},
    str::FromStr,
};
use tempfile::TempDir;
use tokio::fs::File as AsyncFile;

/// Which services a deploy builds, and how.
#[derive(Args)]
pub struct Deploy {
    #[arg(default_value = ".")]
    pub dir: PathBuf,
    /// This service of the workspace in place of the directory's; repeatable.
    #[arg(short = 'p', long = "package")]
    pub packages: Vec<String>,
    /// The registry repository to push to, in place of `image` in
    /// `crab.toml`.
    #[arg(long)]
    pub image: Option<String>,
    /// Deploy even when it breaks what callers used, or undoes a cron its
    /// owners set.
    #[arg(long)]
    pub force: bool,
    /// Write the crons its owners set into `[crons]` in `crab.toml` before
    /// deploying.
    #[arg(long, conflicts_with = "force")]
    pub pull: bool,
}

/// Deploy the services of the workspace `deploy.dir` is in that it selects,
/// each as [`one`] does, stopping at the first that fails. `stopped` installs
/// them to serve once started.
pub fn run(client: &Client, deploy: &Deploy, stopped: bool) -> Result<()> {
    let Deploy {
        dir,
        packages,
        image,
        force,
        pull,
    } = deploy;
    let (image, force, pull) = (image.as_deref(), *force, *pull);
    let workspace = Workspace::read(dir, project::FILE, packages)?;
    if workspace.services.is_empty() {
        bail!(
            "no {} in the workspace at {} names a service to deploy",
            project::FILE,
            workspace.root.display()
        );
    }
    if workspace.services.len() > 1 && (image.is_some() || pull) {
        bail!("--image and --pull take one service; pick it with -p");
    }
    for name in &workspace.skipped {
        eprintln!("skipped   {name}: deploy = false");
    }
    if client.base == dev::url("") {
        dev::up()?;
    }
    for dir in &workspace.services {
        one(client, dir, image, force, pull, stopped)?;
    }
    Ok(())
}

/// Deploy the project in `dir` as its `crab.toml` names it. On Cloud, the image
/// is pushed to `image`, or `crab.toml`'s, tagged with the crate's version, and
/// deployed from there, and a service deployed for the first time starts from
/// `[database] seed`. With `pull`, the crons the service's owners set on the
/// host are written into `crab.toml`'s `[crons]` first.
fn one(
    client: &Client,
    dir: &Path,
    image: Option<&str>,
    force: bool,
    pull: bool,
    stopped: bool,
) -> Result<()> {
    let mut project = project::read(dir)?;
    if !project.deploy {
        bail!(
            "{}'s {} says deploy = false",
            project.name.as_deref().unwrap_or("this program"),
            project::FILE
        );
    }
    if pull {
        let live = utils::wait(client.crons(project.name()?))?;
        let crons = live
            .crons
            .into_iter()
            .filter_map(|job| Some((job.tool, job.cron?)))
            .collect();
        project::write_crons(dir, &crons)?;
        project = project::read(dir)?;
    }
    let built = berm_build::build(dir, None, project::FILE)?;
    let bytes = fs::read(&built).with_context(|| format!("cannot read {}", built.display()))?;
    let name = project.name()?;
    let deployed = match &client.owner {
        None if project.database.seed.is_some() => {
            bail!(
                "a seed starts a service on Cloud; the local host's own database is where one is made"
            )
        }
        None => utils::wait(client.deploy_image(
            name,
            bytes,
            &project.public,
            force,
            project.crons.as_ref(),
            project.database.size(),
            stopped,
        ))?,
        Some(_) => {
            let image = image.or(project.image.as_deref()).with_context(|| {
                format!(
                    "Cloud deploys through a registry; name one as `image` in {}",
                    project::FILE
                )
            })?;
            let reference = format!("{image}:{}", Manifest::from_image(&bytes)?.version);
            push(&reference, &bytes)?;
            let seed = match &project.database.seed {
                Some(seed) if first(client, name)? => Some(upload(client, name, &dir.join(seed))?),
                _ => None,
            };
            utils::wait(client.deploy(
                name,
                &DeployRequest {
                    reference,
                    public: project.public.clone(),
                    force,
                    crons: project.crons.clone(),
                    size: project.database.size(),
                    seed,
                    stopped,
                },
            ))?
        }
    };
    // The host compiled it to answer, so what comes back is what it will
    // serve — worth showing rather than saying "ok".
    let runtime = utils::wait(client.runtime(name))?;
    inspect::show(
        &deployed.program,
        &runtime,
        &utils::wait(client.terms(name))?,
    );
    for broken in &deployed.breaks {
        println!("  breaks  {broken}");
    }
    let set = utils::wait(client.secrets(name))?;
    let unset: Vec<&String> = runtime
        .deps
        .secrets
        .iter()
        .filter(|secret| !set.iter().any(|one| one.name == **secret))
        .collect();
    for secret in &unset {
        println!("  unset   secret {secret}");
    }
    if !unset.is_empty() {
        match client.owner {
            None => println!("  have a person set them at their terminal: crab secret fill {name}"),
            Some(_) => println!("  ask a person to set them: crab secret request {name}"),
        }
    }
    Ok(())
}

/// Whether `name` is not deployed yet.
fn first(client: &Client, name: &str) -> Result<bool> {
    match utils::wait(client.runtime(name)) {
        Ok(_) => Ok(false),
        Err(Error::Api { status: 404, .. }) => Ok(true),
        Err(e) => Err(e.into()),
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
    let request = SeedRequest {
        sha256: sha256.clone(),
        size,
    };
    if let Some(url) = utils::wait(client.seed(name, &request))?.upload {
        eprintln!("uploading the seed, {size} bytes");
        let file = utils::wait(AsyncFile::open(&copy))?;
        utils::wait(client.upload_seed(&url, file, size))?;
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

/// Upload `image` as `reference`, answering with its digest.
fn push(reference: &str, image: &[u8]) -> Result<String> {
    let reference = Reference::from_str(reference)?;
    Registry::open(&reference, Access::Write)?.push(&reference.reference, image)
}
