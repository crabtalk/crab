//! `crab deploy` — build a project and hand the context's host its image.

use crate::{
    cmd::inspect,
    project::{self, FILE, Project},
    utils,
};
use anyhow::{Context, Result};
use berm_api::Manifest;
use berm_oci::{Access, Reference, Registry};
use crabtalk_sdk::{client::Client, sandbox::service::DeployRequest};
use std::{path::Path, str::FromStr};

/// Deploy the project in `dir` as its `berm.toml` names it. On Cloud, the image
/// is pushed to `image`, or `berm.toml`'s, tagged with the crate's version, and
/// deployed from there. With `pull`, the crons the service's owners set on the
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
            utils::wait(client.deploy(
                &project.name,
                &DeployRequest {
                    reference,
                    public: project.public.clone(),
                    force,
                    crons: project.crons.clone(),
                    size: project.database.size.0,
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

/// Upload `image` as `reference`, answering with its digest.
fn push(reference: &str, image: &[u8]) -> Result<String> {
    let reference = Reference::from_str(reference)?;
    let section = Manifest::section(image)?;
    Registry::open(&reference, Access::Write)?.push(&reference.reference, image, section)
}
