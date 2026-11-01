//! Deploying, removing, and invoking.

use crate::host::Node;
use axum::http::StatusCode;
use berm_api::{
    Break, Manifest, digest,
    host::{self, Deploy},
};
use berm_host::{Denied, Images, Installed, deny};
use berm_oci::{Access, Reference, Registry};
use crabtalk_sdk::services::{price, service::DeployRequest};
use std::str::FromStr;

impl Node {
    /// Registry images are cached locally before installation.
    pub async fn prepare(&self, deploy: DeployRequest) -> Result<host::Deploy, Denied> {
        if deploy.publish_only || deploy.source.is_some() || deploy.create {
            return Err(deny(
                StatusCode::BAD_REQUEST,
                "template publication and copies require Cloud",
            ));
        }
        if deploy.seed.is_some() {
            return Err(deny(
                StatusCode::BAD_REQUEST,
                "a seed starts a service on Cloud; the local host's own database is where one is made",
            ));
        }
        let digest = if deploy.reference.starts_with(digest::PREFIX) {
            deploy.reference.clone()
        } else {
            let reference = Reference::from_str(&deploy.reference)
                .map_err(|e| deny(StatusCode::BAD_REQUEST, format!("{e:#}")))?;
            let bytes = tokio::task::spawn_blocking(move || {
                let registry = Registry::open(&reference, Access::Read)?;
                let (digest, _) = registry.describe(&reference.reference)?;
                registry.blob(&digest)
            })
            .await
            .map_err(|e| deny(StatusCode::BAD_GATEWAY, e.to_string()))?
            .map_err(|e| deny(StatusCode::BAD_GATEWAY, format!("{e:#}")))?;
            self.keep(&bytes).await?
        };
        Ok(host::Deploy {
            digest,
            reference: deploy.reference,
            force: deploy.force,
            crons: deploy.crons,
            size: deploy.size.unwrap_or_else(host::database_size),
            stopped: deploy.stopped,
            check_secrets: true,
            project: deploy.project,
            exempt: Vec::new(),
            vars: deploy.vars,
        })
    }

    /// Make the tools of the image `deploy` names reachable under `name`,
    /// answering with how it breaks callers of the service it replaced.
    /// Refused as the host refuses a deploy.
    pub async fn deploy(
        &self,
        name: &str,
        deploy: Deploy,
    ) -> Result<(Installed, Vec<Break>), Denied> {
        validate(name)?;
        let replaced = self.get(name);
        let installed = self.host.deploy(name, deploy, None).await?;
        let breaks = replaced
            .map(|replaced| replaced.manifest.breaks(&installed.service.manifest))
            .unwrap_or_default();
        for broken in &breaks {
            tracing::warn!(name, "breaks callers: {broken}");
        }
        let runtime = self.host.runtime(name).await?;
        let mut unresolved = runtime.unresolved;
        unresolved
            .secrets
            .retain(|secret| !runtime.deps.optional(secret));
        if !unresolved.is_empty() {
            tracing::warn!(name, "nothing here answers to {unresolved}");
        }
        Ok((installed, breaks))
    }

    /// Deploy `members`, each a name and the digest of an image kept in
    /// `images`, together, as [`Host::deploy_set`] does, answering with how each
    /// breaks callers of the service it replaced.
    ///
    /// [`Host::deploy_set`]: berm_host::Host::deploy_set
    pub async fn deploy_set(
        &self,
        members: Vec<(String, Deploy)>,
    ) -> Result<Vec<(String, Installed, Vec<Break>)>, Denied> {
        let mut replaced = Vec::new();
        for (name, _) in &members {
            validate(name)?;
            replaced.push((name.clone(), self.get(name)));
        }
        let installed = self.host.deploy_set(members).await?;
        Ok(installed
            .into_iter()
            .map(|(name, installed)| {
                let breaks = replaced
                    .iter()
                    .find(|(one, _)| *one == name)
                    .and_then(|(_, replaced)| replaced.as_ref())
                    .map(|replaced| replaced.manifest.breaks(&installed.service.manifest))
                    .unwrap_or_default();
                (name, installed, breaks)
            })
            .collect())
    }

    /// Refused naming the first of `priced` that is not a public tool of the
    /// kept image `digest` names.
    pub async fn priced(&self, digest: &str, priced: &[String]) -> Result<(), Denied> {
        if priced.is_empty() {
            return Ok(());
        }
        let image = self
            .images
            .pull(digest, digest)
            .await
            .map_err(|e| deny(StatusCode::BAD_REQUEST, format!("{e:#}")))?;
        let manifest = Manifest::try_from(image.as_slice())
            .map_err(|e| deny(StatusCode::BAD_REQUEST, format!("{e:#}")))?;
        price::check(&manifest, priced).map_err(|e| deny(StatusCode::BAD_REQUEST, e))
    }

    /// Keep `image` for a deploy to name, answering its digest.
    pub async fn keep(&self, image: &[u8]) -> Result<String, Denied> {
        Ok(self.images.put(image).await?)
    }

    /// Whether the image `digest` names is kept.
    pub async fn holds(&self, digest: &str) -> bool {
        self.images.holds(digest).await
    }

    /// Stop running a service and drop what it committed. `false` if it wasn't
    /// deployed.
    pub async fn undeploy(&self, name: &str) -> Result<bool, Denied> {
        Ok(self.host.remove(name).await?)
    }

    /// Start or stop a service. `false` if it wasn't deployed.
    pub async fn run(&self, name: &str, running: bool) -> Result<bool, Denied> {
        Ok(match running {
            true => self.host.start(name).await?,
            false => self.host.stop(name).await?,
        })
    }
}

/// A service name is lowercase letters, digits, `-` and `_`.
pub(super) fn validate(name: &str) -> Result<(), Denied> {
    if name.is_empty() {
        return Err(deny(
            StatusCode::BAD_REQUEST,
            "a service name cannot be empty",
        ));
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
    {
        return Err(deny(
            StatusCode::BAD_REQUEST,
            format!("service name {name:?} is not lowercase letters, digits, `-` or `_`"),
        ));
    }
    Ok(())
}
