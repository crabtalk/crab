//! Deploy source builds or published images, and publish source releases.

use crate::{
    cmd::{dev, inspect, request},
    project, runtime,
};
use anyhow::{Context, Result, bail};
use berm_api::{Deployed, Manifest, digest, reference::Reference as ImageReference};
use berm_oci::{Access, Reference, Registry};
use clap::Args;
use crabtalk_sdk::{
    account::request::RequestStatus,
    client::Client,
    services::service::{
        DeployRequest, DeployResult, DeploySetRequest, TemplateSource, UploadRequest,
    },
};
use rusqlite::{Connection, OpenFlags};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File},
    io::Read,
    path::{Path, PathBuf},
    str::FromStr,
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
    /// Release notes for template publication, at most 2000 characters.
    #[arg(long, conflicts_with = "from")]
    pub notes: Option<String>,
    /// Provider value for the deploy page, SERVICE:KEY=VALUE; repeatable.
    #[arg(long = "copy", value_name = "SERVICE:KEY=VALUE")]
    pub copy: Vec<String>,
}

/// Deploy selected workspace services, checking every member before switching.
pub fn run(
    client: &Client,
    deploy: &Deploy,
    stopped: bool,
    yes: bool,
    context_registry: Option<&str>,
    publish_only: bool,
) -> Result<()> {
    if publish_only && client.owner.is_none() {
        bail!("template publication requires a Cloud context");
    }
    if !publish_only && deploy.notes.is_some() {
        bail!("--notes applies to crab publish");
    }
    let copy_values = copies(&deploy.copy)?;
    if (yes || client.owner.is_none()) && !copy_values.is_empty() {
        bail!("--copy supplies values for the Cloud deploy page; deploy without --yes");
    }
    if publish_only
        && (deploy.from.is_some()
            || !copy_values.is_empty()
            || deploy.version.is_some()
            || deploy.auto_upgrade)
    {
        bail!(
            "publish builds local projects; --from, --version, --auto-upgrade and --copy apply to deployments"
        );
    }
    // An explicit remote source does not require a directory or Cargo project.
    if let Some(from) = &deploy.from {
        let (name, request) =
            deploy.template_request(client, from, deploy.version.as_deref(), stopped)?;
        return submit(client, BTreeMap::from([(name, request)]), copy_values, yes);
    }
    let selection = project::Selection::read(
        &deploy.dir,
        &deploy.packages,
        publish_only || deploy.build,
        publish_only,
    )?;
    if selection.services.is_empty() {
        bail!("no service at or below {}", deploy.dir.display());
    }
    for name in &selection.skipped {
        eprintln!("skipped   {name}: deploy = false");
    }
    if publish_only {
        return deploy.publish(client, selection, yes, context_registry);
    }
    if client.base == dev::url("")
        && selection
            .services
            .iter()
            .any(|service| deploy.build || service.project.image.is_none())
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
        let mut request = if !deploy.build
            && let Some(image) = &service.project.image
        {
            let from = image.address();
            let version = match &deploy.version {
                Some(version) => version.clone(),
                None => service.version()?,
            };
            deploy
                .template_request(client, &from, Some(&version), stopped)?
                .1
        } else {
            if deploy.version.is_some() || deploy.auto_upgrade {
                bail!("--version and --auto-upgrade require a published image");
            }
            let built = berm_build::build(&service.dir, None)?;
            let bytes =
                fs::read(&built).with_context(|| format!("cannot read {}", built.display()))?;
            let reference = reference(
                client,
                &name,
                &service
                    .project
                    .image
                    .as_ref()
                    .map_or_else(|| name.clone(), |image| image.address()),
                deploy.registry.as_deref().or(context_registry),
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
                stopped,
                force: deploy.force,
                prices: project::prices(&service.project)?,
                ..DeployRequest::default()
            }
        };
        request.project = selection.project.clone();
        if services.insert(name.clone(), request).is_some() {
            bail!("more than one selected service deploys as {name}");
        }
    }
    submit(client, services, copy_values, yes)
}

fn submit(
    client: &Client,
    mut services: BTreeMap<String, DeployRequest>,
    copy_values: BTreeMap<String, BTreeMap<String, String>>,
    yes: bool,
) -> Result<()> {
    if let Some(owner) = &client.owner
        && !yes
    {
        return approved(client, owner.name(), services, copy_values);
    }
    let deployed = match services.pop_first() {
        Some((name, request)) if services.is_empty() => {
            let deployed = runtime::wait(client.deploy(&name, &request))?;
            BTreeMap::from([(name, DeployResult::from(deployed))])
        }
        first => {
            services.extend(first);
            runtime::wait(client.deploy_set(&DeploySetRequest {
                services,
                partial: !yes,
            }))?
        }
    };
    report_results(client, &deployed)
}

fn report_results(client: &Client, results: &BTreeMap<String, DeployResult>) -> Result<()> {
    for (name, deployed) in results {
        match deployed {
            DeployResult::Live(deployed) => report(client, name, deployed)?,
            DeployResult::Published { digest, published } => {
                println!(
                    "{name}: template {} ({digest})",
                    if *published {
                        "published"
                    } else {
                        "unpublished"
                    }
                );
            }
            DeployResult::Failed { failure, explain } => {
                eprintln!("{name}: {failure} {}", explain.as_deref().unwrap_or(""))
            }
        }
    }
    if results
        .values()
        .any(|one| matches!(one, DeployResult::Failed { .. }))
    {
        bail!("some services failed");
    }
    Ok(())
}

/// Ask a person to approve deploying `services` on Cloud, print the page that
/// does it as the last line, and wait for them; then report each service the
/// deploy put live. Fails when they decline, it expires, or it does not go
/// live.
fn approved(
    client: &Client,
    account: &str,
    services: BTreeMap<String, DeployRequest>,
    copy: BTreeMap<String, BTreeMap<String, String>>,
) -> Result<()> {
    let asked = runtime::wait(client.ask_deploy_for(account, services, copy, ""))?;
    let minutes = (asked.expires_at - asked.created_at) / 60;
    eprintln!(
        "a person approves this deploy at the link, signed in, within {minutes} minutes; automation requires a key with deployments:write scope"
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
    let mut results = settled.results;
    for (name, deployed) in settled.deployed {
        results.entry(name).or_insert_with(|| deployed.into());
    }
    report_results(client, &results)
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

impl Deploy {
    fn template_request(
        &self,
        client: &Client,
        from: &str,
        version: Option<&str>,
        stopped: bool,
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
                stopped,
                ..DeployRequest::default()
            },
        ))
    }

    fn publish(
        &self,
        client: &Client,
        selection: project::Selection,
        yes: bool,
        context_registry: Option<&str>,
    ) -> Result<()> {
        let mut groups: BTreeMap<String, BTreeMap<String, DeployRequest>> = BTreeMap::new();
        let mut addresses = BTreeSet::new();
        for service in &selection.services {
            let image = service.project.image.as_ref().with_context(|| {
                format!(
                    "{}: publication requires image = \"owner/service\"",
                    service.dir.display()
                )
            })?;
            if !addresses.insert(image.address()) {
                bail!("more than one selected service publishes to {image}");
            }
            service.source_version()?;
        }
        for service in selection.services {
            let image = service
                .project
                .image
                .as_ref()
                .expect("publication address checked");
            let built = berm_build::build(&service.dir, None)?;
            let bytes =
                fs::read(&built).with_context(|| format!("cannot read {}", built.display()))?;
            let reference = reference(
                client,
                &image.address(),
                &image.address(),
                self.registry.as_deref().or(context_registry),
                bytes,
            )?;
            let request = DeployRequest {
                reference,
                template: Some(service.project.template),
                release_notes: self.notes.clone(),
                publish_only: true,
                ..DeployRequest::default()
            };
            groups
                .entry(image.owner.clone())
                .or_default()
                .insert(image.name.clone(), request);
        }
        for (owner, services) in groups {
            if !yes {
                approved(client, &owner, services, BTreeMap::new())?;
            } else {
                let results = runtime::wait(client.publish_set(
                    &owner,
                    &DeploySetRequest {
                        services,
                        partial: true,
                    },
                ))?;
                let results = results
                    .into_iter()
                    .map(|(name, result)| (format!("{owner}/{name}"), result))
                    .collect();
                report_results(client, &results)?;
            }
        }
        Ok(())
    }
}

fn reference(
    client: &Client,
    name: &str,
    repository: &str,
    registry: Option<&str>,
    bytes: Vec<u8>,
) -> Result<String> {
    match registry {
        Some(registry) => {
            let version = Manifest::try_from(bytes.as_slice())?.version;
            let reference = Reference::from_str(&format!("{registry}/{repository}:{version}"))?;
            let digest =
                Registry::open(&reference, Access::Write)?.push(&reference.reference, &bytes)?;
            Ok(format!(
                "{}/{}@{digest}",
                reference.registry, reference.repository
            ))
        }
        None => send(client, name, bytes),
    }
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
