//! `crab deploy` and `crab push` — build services and hand the context's host
//! their images, serving or stopped.

use crate::{
    cmd::{dev, inspect, request},
    project, runtime,
};
use anyhow::{Context, Result, bail};
use berm_api::{Deployed, Manifest, digest, project::Project};
use berm_build::Workspace;
use berm_oci::{Access, Reference, Registry};
use clap::Args;
use crabtalk_sdk::{
    account::request::RequestStatus,
    client::Client,
    services::service::{DeployRequest, DeploySetRequest, UploadRequest},
};
use rusqlite::{Connection, OpenFlags};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
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
    /// Push every service's image to this registry, in place of the one
    /// its image names, as `localhost:5001`.
    #[arg(long)]
    pub registry: Option<String>,
    /// Deploy even when it breaks what callers used, or undoes a cron its
    /// owners set.
    #[arg(long)]
    pub force: bool,
    /// Write the crons its owners set into `[crons]` in `crab.toml` before
    /// deploying.
    #[arg(long, conflicts_with = "force")]
    pub pull: bool,
    /// Deploy another account's public service, `owner/name`, as your own
    /// `name`, from the registry image it runs; on Cloud.
    #[arg(long, value_name = "OWNER/NAME", conflicts_with_all = ["packages", "image", "registry", "pull"])]
    pub from: Option<String>,
    /// Provider value for the deploy page, SERVICE:KEY=VALUE; repeatable.
    #[arg(long = "copy", value_name = "SERVICE:KEY=VALUE")]
    pub copy: Vec<String>,
}

/// Deploy the services of the workspace `deploy.dir` is in that it selects,
/// more than one together, every one checked before any switches; with
/// `pull`, one service's crons its owners set on the host are written into
/// `crab.toml`'s `[crons]` first. `stopped` installs them to serve once
/// started. On Cloud, a person approves the deploy at a link unless `yes`.
pub fn run(client: &Client, deploy: &Deploy, stopped: bool, yes: bool) -> Result<()> {
    let Deploy {
        dir,
        packages,
        image,
        registry,
        force,
        pull,
        from,
        copy: copy_values,
    } = deploy;
    let copy_values = copies(copy_values)?;
    if (yes || client.owner.is_none()) && !copy_values.is_empty() {
        bail!("--copy supplies values for the Cloud deploy page; deploy without --yes");
    }
    if let Some(from) = from {
        return copy(client, from, *force, stopped, yes, copy_values);
    }
    let (image, registry, force, pull) = (image.as_deref(), registry.as_deref(), *force, *pull);
    let workspace = Workspace::read(dir, packages)?;
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
    // The project its services are deployed in: the workspace directory's
    // name.
    let in_project = workspace
        .root
        .file_name()
        .and_then(|name| name.to_str())
        .with_context(|| format!("{} names no project", workspace.root.display()))?
        .to_owned();
    let one = workspace.services.len() == 1;
    let mut services = BTreeMap::new();
    for dir in &workspace.services {
        // On Cloud, the image is pushed to `image`, or `crab.toml`'s, tagged
        // with the crate's version, or uploaded when neither names a registry.
        let (project, bytes) = build(client, dir, one && pull)?;
        let name = project.name()?.to_owned();
        let image = image.filter(|_| one);
        let reference = reference(client, &name, &project, image, registry, bytes)?;
        let request = request(
            client,
            dir,
            &project,
            reference,
            &in_project,
            force,
            stopped,
        )?;
        services.insert(name, request);
    }
    if client.owner.is_some() && !yes {
        return approved(client, services, copy_values);
    }
    let deployed = match services.pop_first() {
        Some((name, request)) if services.is_empty() => {
            let deployed = runtime::wait(client.deploy(&name, &request))?;
            BTreeMap::from([(
                name,
                crabtalk_sdk::services::service::DeployResult::from(deployed),
            )])
        }
        first => {
            services.extend(first);
            runtime::wait(client.deploy_set(&DeploySetRequest {
                services,
                partial: !yes,
            }))?
        }
    };
    for (name, deployed) in &deployed {
        match deployed {
            crabtalk_sdk::services::service::DeployResult::Live(deployed) => {
                report(client, name, deployed)?
            }
            crabtalk_sdk::services::service::DeployResult::Failed { failure, explain } => {
                eprintln!("{name}: {failure} {}", explain.as_deref().unwrap_or(""))
            }
        }
    }
    if deployed.values().any(|one| {
        matches!(
            one,
            crabtalk_sdk::services::service::DeployResult::Failed { .. }
        )
    }) {
        bail!("some services did not deploy");
    }
    Ok(())
}

/// Ask a person to approve deploying `services` on Cloud, print the page that
/// does it as the last line, and wait for them; then report each service the
/// deploy put live. Fails when they decline, it expires, or it does not go
/// live.
fn approved(
    client: &Client,
    services: BTreeMap<String, DeployRequest>,
    copy: BTreeMap<String, BTreeMap<String, String>>,
) -> Result<()> {
    let asked = runtime::wait(client.ask_deploy_with_copy(services, copy, ""))?;
    let minutes = (asked.expires_at - asked.created_at) / 60;
    eprintln!(
        "a person approves this deploy at the link, signed in, within {minutes} minutes; automation requires a key with auto scope"
    );
    println!("{}", asked.url);
    let settled = request::settled(client, &asked.code, (minutes * 60) as u64)?;
    match settled.status {
        RequestStatus::Pending => bail!("nobody approved the deploy in {minutes} minutes"),
        RequestStatus::Deploying => {
            bail!("the deploy is still running; check its page for results")
        }
        RequestStatus::Declined => bail!("the person declined the deploy"),
        RequestStatus::Expired => {
            bail!("the deploy request expired; check its page for results before deploying again")
        }
        RequestStatus::Completed => {}
    }
    if let Some(failure) = &settled.failure {
        match &settled.explain {
            Some(code) => bail!("the approved deploy did not go live [{code}]: {failure}"),
            None => bail!("the approved deploy did not go live: {failure}"),
        }
    }
    for (name, deployed) in &settled.deployed {
        report(client, name, deployed)?;
    }
    let mut failed = false;
    for (name, one) in &settled.results {
        if let crabtalk_sdk::services::service::DeployResult::Failed { failure, explain } = one {
            eprintln!("{name}: {failure} {}", explain.as_deref().unwrap_or(""));
            failed = true;
        }
    }
    if failed {
        bail!("some services did not deploy");
    }
    Ok(())
}

fn copies(values: &[String]) -> Result<BTreeMap<String, BTreeMap<String, String>>> {
    let mut copy = BTreeMap::<String, BTreeMap<String, String>>::new();
    for value in values {
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

/// Deploy another account's registry image through the same approval flow.
fn copy(
    client: &Client,
    from: &str,
    force: bool,
    stopped: bool,
    yes: bool,
    copy: BTreeMap<String, BTreeMap<String, String>>,
) -> Result<()> {
    if client.owner.is_none() {
        bail!("--from deploys from a registry, which only Cloud pulls");
    }
    let (_, name) = from.split_once('/').context("--from takes owner/name")?;
    let listing = runtime::wait(client.listing(from))?;
    let reference = listing
        .reference
        .context("the source image was uploaded; only its owner can deploy it")?;
    let request = DeployRequest {
        reference,
        force,
        crons: None,
        size: None,
        seed: None,
        stopped,
        project: None,
        prices: BTreeMap::new(),
        vars: BTreeMap::new(),
    };
    if !yes {
        return approved(client, BTreeMap::from([(name.to_owned(), request)]), copy);
    }
    let deployed = runtime::wait(client.deploy(name, &request))?;
    report(client, name, &deployed)
}

/// Read and build the project in `dir`, refused when its `crab.toml` says
/// `deploy = false`. With `pull`, the
/// crons its owners set on the host are written into its `[crons]` first.
fn build(client: &Client, dir: &Path, pull: bool) -> Result<(Project, Vec<u8>)> {
    let mut project = project::read(dir)?;
    if !project.deploy {
        bail!(
            "{}'s {} says deploy = false",
            project.name.as_deref().unwrap_or("this service"),
            project::FILE
        );
    }
    if pull {
        let live = runtime::wait(client.crons(project.name()?))?;
        let crons = live
            .crons
            .into_iter()
            .filter_map(|job| Some((job.tool, job.cron?)))
            .collect();
        project::write_crons(dir, &crons)?;
        project = project::read(dir)?;
    }
    let built = berm_build::build(dir, None)?;
    let bytes = fs::read(&built).with_context(|| format!("cannot read {}", built.display()))?;
    Ok((project, bytes))
}

/// Where the host deploys `name` from: on Cloud, the image pushed to `image`,
/// or `crab.toml`'s, tagged with the crate's version, on `registry` in place
/// of its own when given; else uploaded to the host.
fn reference(
    client: &Client,
    name: &str,
    project: &Project,
    image: Option<&str>,
    registry: Option<&str>,
    bytes: Vec<u8>,
) -> Result<String> {
    match image.or(project.image.as_deref()) {
        Some(image) if client.owner.is_some() => {
            let mut reference = Reference::from_str(&format!(
                "{image}:{}",
                Manifest::try_from(bytes.as_slice())?.version
            ))?;
            if let Some(registry) = registry {
                reference.registry = registry.to_ascii_lowercase();
            }
            let reference = reference.to_string();
            push(&reference, &bytes)?;
            Ok(reference)
        }
        _ => send(client, name, bytes),
    }
}

/// The deploy of `project`, in `dir`, from `reference`, in the project
/// `workspace`; a service deployed to Cloud for the first time starts from
/// `[database] seed`.
fn request(
    client: &Client,
    dir: &Path,
    project: &Project,
    reference: String,
    workspace: &str,
    force: bool,
    stopped: bool,
) -> Result<DeployRequest> {
    let name = project.name()?;
    let seed = match &project.database.seed {
        Some(_) if client.owner.is_none() => bail!(
            "{name}: a seed starts a service on Cloud; the local host's own database is where one is made"
        ),
        Some(seed) if runtime::wait(first(client, name))? => {
            Some(upload(client, name, &dir.join(seed))?)
        }
        _ => None,
    };
    Ok(DeployRequest {
        reference,
        force,
        crons: project.crons.clone(),
        size: Some(project.database.size()),
        seed,
        stopped,
        project: Some(workspace.to_owned()),
        prices: project::prices(project)?,
        vars: BTreeMap::new(),
    })
}

/// Show what `name` serves now that `deployed` installed it, how it breaks
/// callers, and the secrets it needs that are not set.
fn report(client: &Client, name: &str, deployed: &Deployed) -> Result<()> {
    // The host compiled it to answer, so what comes back is what it will
    // serve.
    let (running, terms, set) = runtime::wait(async {
        tokio::try_join!(
            client.runtime(name),
            client.terms(name),
            client.secrets(name)
        )
    })?;
    inspect::show(&deployed.service, Some(&running), &terms, None);
    for broken in &deployed.breaks {
        println!("  breaks  {broken}");
    }
    let unset: Vec<&String> = running
        .deps
        .secrets
        .iter()
        .filter(|secret| !set.iter().any(|one| one.name == **secret && one.sent()))
        .collect();
    for secret in &unset {
        println!("  unset   secret {secret}");
    }
    Ok(())
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

/// Upload `image` for `name`'s deploy unless the host holds it already,
/// answering the digest the deploy names it by.
fn send(client: &Client, name: &str, image: Vec<u8>) -> Result<String> {
    let sha256 = digest::hex(&image);
    let size = image.len() as u64;
    let request = UploadRequest {
        sha256: sha256.clone(),
        size,
    };
    if let Some(url) = runtime::wait(client.image(name, &request))?.upload {
        eprintln!("uploading the image, {size} bytes");
        runtime::wait(client.upload(&url, image, size))?;
    }
    Ok(digest::prefixed(&sha256))
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
