//! Deploying, removing, and invoking.

use crate::host::Service;
use axum::http::StatusCode;
use berm_api::{Break, Output, host::Deploy};
use berm_host::{Denied, Installed, deny};

impl Service {
    /// Make the tools of the image `deploy` names reachable under `name`,
    /// answering with how it breaks callers of the program it replaced.
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
            .map(|replaced| replaced.manifest().breaks(installed.program.manifest()))
            .unwrap_or_default();
        for broken in &breaks {
            tracing::warn!(name, "breaks callers: {broken}");
        }
        let unresolved = self.host.runtime(name).await?.unresolved;
        if !unresolved.is_empty() {
            tracing::warn!(name, "nothing here answers to {unresolved}");
        }
        let _ = self.changed.send(());
        Ok((installed, breaks))
    }

    /// Deploy `members`, each a name and the digest of an image kept in
    /// `images`, together, as [`Host::deploy_set`] does, answering with how each
    /// breaks callers of the program it replaced.
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
        let _ = self.changed.send(());
        Ok(installed
            .into_iter()
            .map(|(name, installed)| {
                let breaks = replaced
                    .iter()
                    .find(|(one, _)| *one == name)
                    .and_then(|(_, replaced)| replaced.as_ref())
                    .map(|replaced| replaced.manifest().breaks(installed.program.manifest()))
                    .unwrap_or_default();
                (name, installed, breaks)
            })
            .collect())
    }

    /// Keep `image` for a deploy to name, answering its digest.
    pub async fn keep(&self, image: &[u8]) -> Result<String, Denied> {
        Ok(self.images.put(image).await?)
    }

    /// Stop running a program and drop what it committed. `false` if it wasn't
    /// deployed.
    pub async fn undeploy(&self, name: &str) -> Result<bool, Denied> {
        let removed = self.host.remove(name).await?;
        if removed {
            let _ = self.changed.send(());
        }
        Ok(removed)
    }

    /// Start or stop a program. `false` if it wasn't deployed.
    pub async fn run(&self, name: &str, running: bool) -> Result<bool, Denied> {
        let set = match running {
            true => self.host.start(name).await?,
            false => self.host.stop(name).await?,
        };
        if set {
            let _ = self.changed.send(());
        }
        Ok(set)
    }

    /// Run one public tool with no caller.
    pub async fn call(&self, name: &str, tool: &str, args: Vec<u8>) -> Result<Output, Denied> {
        let (_, output) = self.host.call(name, None, tool, args, false).await?;
        Ok(output)
    }
}

/// A name has to survive being half of an MCP tool name. The dot is the one
/// that matters: dispatch splits `{program}.{tool}` on the first one, so a
/// program carrying a dot would eat its own tool.
fn validate(name: &str) -> Result<(), Denied> {
    if name.is_empty() {
        return Err(deny(
            StatusCode::BAD_REQUEST,
            "a program name cannot be empty",
        ));
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
    {
        return Err(deny(
            StatusCode::BAD_REQUEST,
            format!("program name {name:?} is not lowercase letters, digits, `-` or `_`"),
        ));
    }
    Ok(())
}
