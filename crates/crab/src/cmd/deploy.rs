//! `crab deploy` — build a project and hand the context's host its image.

use crate::{cmd::inspect, project, utils};
use anyhow::{Context, Result, bail};
use berm_api::{Manifest, project::Project};
use berm_oci::{Access, Reference, Registry};
use crabtalk_sdk::{
    client::{Client, Error},
    sandbox::service::{DeployRequest, SeedRequest},
};
use rusqlite::{Connection, OpenFlags};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File},
    io::Read,
    path::Path,
    str::FromStr,
};
use tempfile::TempDir;
use tokio::fs::File as AsyncFile;

/// Deploy the project in `dir` as its `berm.toml` names it. On Cloud, the image
/// is pushed to `image`, or `berm.toml`'s, tagged with the crate's version, and
/// deployed from there, and a service deployed for the first time starts from
/// `[database] seed`. With `pull`, the crons the service's owners set on the
/// host are written into `berm.toml`'s `[crons]` first.
pub fn run(
    client: &Client,
    dir: &Path,
    image: Option<&str>,
    force: bool,
    pull: bool,
) -> Result<()> {
    let mut project = Project::read(dir)?;
    if !project.deploy {
        bail!(
            "{}'s {} says deploy = false",
            project.name.as_deref().unwrap_or("this program"),
            Project::FILE
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
        project = Project::read(dir)?;
    }
    let built = berm_build::build(dir, None)?;
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
        ))?,
        Some(_) => {
            let image = image.or(project.image.as_deref()).with_context(|| {
                format!(
                    "Cloud deploys through a registry; name one as `image` in {}",
                    Project::FILE
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
