use crate::{deployment, project, runtime};
use anyhow::{Context, Result, bail};
use clap::Args;
use crabtalk_sdk::{
    client::Client,
    services::service::{DeployRequest, DeploySetRequest},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::PathBuf,
};

#[derive(Args)]
pub struct Push {
    /// Service directory or parent directory containing services.
    #[arg(default_value = ".")]
    pub dir: PathBuf,
    /// Select this service by name across the workspace; repeatable.
    #[arg(short = 'p', long = "package")]
    pub packages: Vec<String>,
    /// Push images to this registry, overriding the context, as `localhost:5001`.
    #[arg(long)]
    pub registry: Option<String>,
    /// Release notes, at most 2000 characters.
    #[arg(long)]
    pub notes: Option<String>,
}

impl Push {
    pub fn run(&self, client: &Client, yes: bool, context_registry: Option<&str>) -> Result<()> {
        if client.owner.is_none() {
            bail!("template publication requires a Cloud context");
        }
        let selection = project::Selection::read(&self.dir, &self.packages, true, true)?;
        if selection.services.is_empty() {
            bail!("no service at or below {}", self.dir.display());
        }
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
            let reference = deployment::reference(
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
                deployment::approved(client, &owner, services, BTreeMap::new())?;
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
                deployment::report_results(client, &results)?;
            }
        }
        Ok(())
    }
}
