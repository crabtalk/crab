//! `crab deploy` — build a project and hand the context's host its image.

use crate::{
    cmd::inspect,
    project::{self, FILE, Project},
    utils,
};
use anyhow::{Context, Result, bail};
use berm_api::Manifest;
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
    if pull {
        let live = utils::wait(client.crons(&project.name))?;
        let crons = live
            .crons
            .into_iter()
            .filter_map(|job| Some((job.tool, job.cron?)))
            .collect();
        project::write_crons(dir, &crons)?;
        project = Project::read(dir)?;
    }
    let bytes = project::build(dir)?;
    let deployed = match &client.owner {
        None if project.database.seed.is_some() => {
            bail!("a seed starts a service on Cloud; bermd's own database is where one is made")
        }
        None => utils::wait(client.deploy_image(
            &project.name,
            bytes,
            &project.public,
            force,
            project.crons.as_ref(),
            project.database.size.0,
        ))?,
        Some(_) => {
            let image = image.or(project.image.as_deref()).with_context(|| {
                format!("Cloud deploys through a registry; name one as `image` in {FILE}")
            })?;
            let reference = format!("{image}:{}", Manifest::from_image(&bytes)?.version);
            push(&reference, &bytes)?;
            let seed = match &project.database.seed {
                Some(seed) if first(client, &project.name)? => {
                    Some(upload(client, &project.name, &dir.join(seed))?)
                }
                _ => None,
            };
            utils::wait(client.deploy(
                &project.name,
                &DeployRequest {
                    reference,
                    public: project.public.clone(),
                    force,
                    crons: project.crons.clone(),
                    size: project.database.size.0,
                    seed,
                },
            ))?
        }
    };
    // The host compiled it to answer, so what comes back is what it will
    // serve — worth showing rather than saying "ok".
    let runtime = utils::wait(client.runtime(&project.name))?;
    inspect::show(
        &deployed.program,
        &runtime,
        &utils::wait(client.terms(&project.name))?,
    );
    for broken in &deployed.breaks {
        println!("  breaks  {broken}");
    }
    // A bermd's secrets are its own file's; Cloud's are set per service.
    if client.owner.is_some() {
        let set = utils::wait(client.secrets(&project.name))?;
        for secret in &runtime.deps.secrets {
            if !set.iter().any(|one| one.name == *secret) {
                println!(
                    "  unset   secret {secret}: printf %s \"$VALUE\" | crab secret put {} {secret}",
                    project.name
                );
            }
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
    let copy = std::env::temp_dir().join(format!("crab-seed-{}.sqlite", std::process::id()));
    let _ = fs::remove_file(&copy);
    // One consistent file, its write-ahead log folded in, while a bermd may
    // hold the database open.
    Connection::open_with_flags(seed, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .with_context(|| format!("opening the seed {}", seed.display()))?
        .execute("VACUUM INTO ?1", [copy.to_string_lossy()])
        .with_context(|| format!("copying the seed {}", seed.display()))?;
    let (sha256, size) = hash(&copy)?;
    let request = SeedRequest {
        sha256: sha256.clone(),
        size,
    };
    if let Some(url) = utils::wait(client.seed(name, &request))?.upload {
        eprintln!("uploading the seed, {size} bytes");
        utils::wait(async {
            let file = AsyncFile::open(&copy).await?;
            client.upload_seed(&url, file, size).await?;
            anyhow::Ok(())
        })?;
    }
    fs::remove_file(&copy)?;
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
    let section = Manifest::section(image)?;
    Registry::open(&reference, Access::Write)?.push(&reference.reference, image, section)
}
