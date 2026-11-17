//! Deploying a service.

use crate::{
    Denied, deny,
    env::{Env, Images, Secrets},
    host::{self, Host},
    record::usage,
    state::service,
};
use axum::http::StatusCode;
use berm::{Service, database::Database};
use berm_api::{
    Deps, Manifest, Surface,
    host::{
        Breaking, Broken, Checked, Deploy, Kind, LastUse, MAX_DATABASE_SIZE, MIN_DATABASE_SIZE,
        Outcome,
    },
    schedule as cron,
};
use std::{path::Path, sync::Arc};

/// The error code a deploy refused for a budget past the host's most
/// carries.
const PAST_MOST: &str = "E0013";

/// The error code a deploy refused for a host call this host does not serve
/// carries.
const MISSING_HOST_CALL: &str = "E0020";

/// The error code a deploy refused for a required secret unset carries.
const SECRETS_UNSET: &str = "E0021";

/// A deploy that went live.
pub struct Installed {
    pub service: Arc<Service>,
    /// The migrations it applied, a kit's as `{kit}/{name}`.
    pub migrations: Vec<String>,
}

impl<E: Env> Host<E> {
    /// Run the index entry `deploy` names as `service`. Refused when it
    /// needs a syscall this host does not serve, or, unless `force`, when it
    /// breaks a public tool or event of the service
    /// it replaces that a caller used, with a call that was done, in the last
    /// [`usage::WINDOW`] days, but by the services `exempt` names. Refused too
    /// when its `size` is outside
    /// [`MIN_DATABASE_SIZE`]..=[`MAX_DATABASE_SIZE`] or below what its
    /// database holds. A refused deploy leaves the database held to the size
    /// it was. A service whose image this host cannot read, such as one built
    /// against another ABI, is replaced as on a first deploy.
    ///
    /// `seed` starts a new service from a SQLite data export. SQL databases
    /// must match the image's migrations; services without SQL accept only
    /// `_berm_state`. A refused SQL deployment removes the installed seed.
    pub async fn deploy(
        &self,
        service: &str,
        deploy: Deploy,
        seed: Option<&Path>,
    ) -> Result<Installed, Denied> {
        let Deploy {
            reference,
            digest,
            force,
            size,
            stopped,
            check_secrets,
            project,
            exempt,
            vars,
        } = deploy;
        let (image, manifest) = self.image(&reference, &digest, size).await?;
        let unset = match check_secrets {
            true => self.unset(service, &manifest.deps).await?,
            false => Vec::new(),
        };
        if !unset.is_empty() {
            return Err(deny(
                StatusCode::CONFLICT,
                format!(
                    "{service}: required secrets are unset: {}",
                    unset.join(", ")
                ),
            )
            .explained(SECRETS_UNSET));
        }
        for (var, value) in &vars {
            manifest
                .deps
                .settable(var, value)
                .map_err(|e| deny(StatusCode::BAD_REQUEST, format!("{service}: {e:#}")))?;
        }

        // The row is read under the lock, so the breaks are against what the
        // service runs, and the row is written only once the image has
        // compiled: a refused image leaves the service on what it ran.
        let key = service.to_owned();
        let (scope, name) = host::split(service);
        let lock = self
            .deploying
            .lock()
            .entry(key.clone())
            .or_default()
            .clone();
        let _deploying = lock.lock().await;
        if seed.is_some() {
            self.seedable(service).await?;
        }
        let current = self.current(service).await?;
        self.moving(service, project.as_deref(), force).await?;
        let cron_tools = self.cron_tools(&manifest).await?;
        if let Some(current) = &current
            && !force
        {
            let breaks = Broken::exempted(self.broken(service, current, &manifest).await?, &exempt);
            if !breaks.is_empty() {
                return Err(deny(StatusCode::CONFLICT, Breaking(&breaks).to_string()));
            }
        }
        let state_seed = seed
            .filter(|_| !manifest.keeps_database())
            .map(Path::to_path_buf);
        if let Some(seed) = seed {
            let bytes = std::fs::metadata(seed)
                .map_err(|error| deny(StatusCode::BAD_REQUEST, error.to_string()))?
                .len();
            if bytes > size {
                return Err(deny(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "The seed is larger than the service's storage size",
                )
                .explained(Denied::SEED_SIZE));
            }
            if manifest.keeps_database() {
                self.databases.seed(&key, seed).map_err(|error| {
                    deny(StatusCode::CONFLICT, error.to_string()).explained(Denied::SEEDED)
                })?;
            } else {
                let path = seed.to_owned();
                tokio::task::spawn_blocking(move || service::check_seed(&path))
                    .await
                    .map_err(|error| deny(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?
                    .map_err(|error| deny(StatusCode::BAD_REQUEST, error.to_string()))?;
            }
        }
        let mut limited: Option<(Arc<Database>, u64)> = None;
        let installed = async {
            let mut migrations = Vec::new();
            if manifest.keeps_database() {
                let database = self
                    .databases
                    .get(&key)
                    .map_err(|e| deny(StatusCode::INTERNAL_SERVER_ERROR, format!("{e:#}")))?;
                let before = database
                    .limit(size)
                    .await
                    .map_err(|e| deny(StatusCode::UNPROCESSABLE_ENTITY, format!("{e:#}")))?;
                limited = Some((database.clone(), before));
                migrations = database
                    .migrate(manifest.migrations())
                    .await
                    .map_err(|e| deny(StatusCode::BAD_REQUEST, format!("{e:#}")))?;
            }
            let deprecated: Vec<String> = manifest
                .tools
                .iter()
                .filter(|spec| spec.deprecated)
                .map(|spec| spec.name.clone())
                .collect();
            let deployed = self.compile(key.clone(), image).await.map_err(|e| {
                let denied = deny(StatusCode::BAD_REQUEST, format!("{e:#}"));
                if e.downcast_ref::<berm::PastMost>().is_some() {
                    denied.explained(PAST_MOST)
                } else if e.downcast_ref::<berm::MissingHostCall>().is_some() {
                    denied.explained(MISSING_HOST_CALL)
                } else if e.is::<berm::MemoryLimit>() {
                    denied.explained(Denied::MEMORY)
                } else {
                    denied
                }
            })?;
            service::install(
                &self.store,
                scope,
                name,
                &reference,
                &digest,
                &manifest.version,
                size,
                deployed.bytes,
                deprecated,
                cron_tools,
                manifest.crons.clone(),
                manifest.workflows.clone(),
                stopped,
                project,
                manifest.deps.vars.keys().cloned().collect(),
                vars,
                (self.now)() as i64,
                state_seed,
            )
            .await?;
            self.stops.mark(&key, stopped);
            Ok(Installed {
                service: deployed,
                migrations,
            })
        }
        .await;
        if installed.is_err()
            && let Some((database, before)) = limited
            && let Err(e) = database.limit(before).await
        {
            tracing::error!(service, error = %e, "the refused size stays");
        }
        if seed.is_some()
            && manifest.keeps_database()
            && installed.is_err()
            && let Err(e) = self.databases.remove(&key)
        {
            tracing::error!(service, error = %e, "the refused seed stays");
        }
        if installed.is_ok() {
            self.waking.notify_one();
            self.scheduling.notify_one();
            if let Err(e) = self.sample(service).await {
                tracing::warn!(service, "sampling usage failed: {e:#}");
            }
        }
        installed
    }

    /// Check `deploy` as [`Host::deploy`] would run it as `service`, and
    /// install nothing: refused as the deploy would be but for its breaks,
    /// which it answers with, the callers `exempt` names left out. `seeded`
    /// checks a seed could start its database. Its migrations are applied to
    /// the service's database and rolled back; a service with no database yet
    /// rehearses none, and the image is not loaded.
    pub async fn check(
        &self,
        service: &str,
        deploy: Deploy,
        seeded: bool,
    ) -> Result<Checked, Denied> {
        let (_, manifest) = self
            .image(&deploy.reference, &deploy.digest, deploy.size)
            .await?;
        if deploy.check_secrets {
            let unset = self.unset(service, &manifest.deps).await?;
            if !unset.is_empty() {
                return Err(deny(
                    StatusCode::CONFLICT,
                    format!(
                        "{service}: required secrets are unset: {}",
                        unset.join(", ")
                    ),
                )
                .explained(SECRETS_UNSET));
            }
        }
        let key = service.to_owned();
        let lock = self
            .deploying
            .lock()
            .entry(key.clone())
            .or_default()
            .clone();
        let _deploying = lock.lock().await;
        if seeded {
            self.seedable(service).await?;
        }
        let current = self.current(service).await?;
        self.moving(service, deploy.project.as_deref(), deploy.force)
            .await?;
        self.cron_tools(&manifest).await?;
        let breaks = match &current {
            Some(current) => Broken::exempted(
                self.broken(service, current, &manifest).await?,
                &deploy.exempt,
            ),
            None => Vec::new(),
        };
        let exists = self.databases.path(&key).exists();
        let mut migrations = Vec::new();
        if manifest.keeps_database() && exists {
            let database = self
                .databases
                .get(&key)
                .map_err(|e| deny(StatusCode::INTERNAL_SERVER_ERROR, format!("{e:#}")))?;
            let before = database
                .limit(deploy.size)
                .await
                .map_err(|e| deny(StatusCode::UNPROCESSABLE_ENTITY, format!("{e:#}")))?;
            let rehearsed = database.rehearse(manifest.migrations()).await;
            if let Err(e) = database.limit(before).await {
                tracing::error!(service, error = %e, "the checked size stays");
            }
            migrations = rehearsed.map_err(|e| deny(StatusCode::BAD_REQUEST, format!("{e:#}")))?;
        }
        Ok(Checked {
            version: manifest.version.clone(),
            migrations,
            breaks,
            secrets: manifest.deps.secrets.clone(),
            required: manifest.deps.unset(&[]),
        })
    }

    /// The image `digest` names and its manifest, refused when `size` is
    /// outside [`MIN_DATABASE_SIZE`]..=[`MAX_DATABASE_SIZE`], the image is not
    /// a service or it needs a syscall this host does not serve.
    async fn image(
        &self,
        reference: &str,
        digest: &str,
        size: u64,
    ) -> Result<(Vec<u8>, Manifest), Denied> {
        if !(MIN_DATABASE_SIZE..=MAX_DATABASE_SIZE).contains(&size) {
            return Err(deny(
                StatusCode::UNPROCESSABLE_ENTITY,
                format!(
                    "a database holds from {MIN_DATABASE_SIZE} to {MAX_DATABASE_SIZE} bytes, not {size}"
                ),
            ));
        }
        let image = self
            .images
            .pull(reference, digest)
            .await
            .map_err(|e| deny(StatusCode::BAD_GATEWAY, format!("{e:#}")))?;
        let manifest = Manifest::try_from(image.as_slice()).map_err(Denied::manifest)?;
        self.served(&manifest)?;
        Ok((image, manifest))
    }

    /// Refuses deploying `service` in `project` when it is in another
    /// project, unless `force`, which moves it.
    async fn moving(
        &self,
        service: &str,
        project: Option<&str>,
        force: bool,
    ) -> Result<(), Denied> {
        let (scope, name) = host::split(service);
        let Some(project) = project else {
            return Ok(());
        };
        let Some(was) = service::row(&self.store, scope, name)
            .await?
            .and_then(|row| row.project)
        else {
            return Ok(());
        };
        if was == project || force {
            return Ok(());
        }
        Err(deny(
            StatusCode::CONFLICT,
            format!(
                "{name} is in project {was}; call it from {project} as it is, rename {project}'s copy, or deploy with `force` to move it into {project}"
            ),
        ))
    }

    /// The service `service` runs, none when it runs none or one this host
    /// cannot read, such as one built against another ABI. Read under its
    /// deploy lock.
    async fn seedable(&self, service: &str) -> Result<(), Denied> {
        let (scope, name) = host::split(service);
        if service::row(&self.store, scope, name).await?.is_some()
            || self.databases.path(service).exists()
        {
            return Err(
                deny(StatusCode::CONFLICT, "A seed only starts a new service")
                    .explained(Denied::SEEDED),
            );
        }
        Ok(())
    }

    async fn current(&self, service: &str) -> Result<Option<Arc<Service>>, Denied> {
        let (scope, name) = host::split(service);
        let Some(row) = service::row(&self.store, scope, name).await? else {
            return Ok(None);
        };
        let (reference, digest) = (row.reference.clone(), row.digest.clone());
        match self.load(row).await {
            Ok(deployed) => Ok(Some(deployed)),
            Err(e) => {
                let image = self
                    .images
                    .pull(&reference, &digest)
                    .await
                    .map_err(|e| deny(StatusCode::BAD_GATEWAY, format!("{e:#}")))?;
                if Manifest::try_from(image.as_slice()).is_ok() {
                    return Err(deny(StatusCode::BAD_GATEWAY, format!("{e:#}")));
                }
                tracing::warn!(service, "replacing a service this host cannot read: {e:#}");
                Ok(None)
            }
        }
    }

    /// The secrets of `deps` a required credential holds that `service` has
    /// none of.
    async fn unset(&self, service: &str, deps: &Deps) -> Result<Vec<String>, Denied> {
        let set = self
            .granted(service, deps)
            .await
            .map_err(|e| deny(StatusCode::INTERNAL_SERVER_ERROR, format!("{e:#}")))?;
        Ok(deps.unset(&set))
    }

    pub(crate) async fn granted(&self, service: &str, deps: &Deps) -> anyhow::Result<Vec<String>> {
        let mut set = Vec::new();
        for name in self.secrets.names(service).await? {
            let mut allowed = true;
            for host in deps.secret_hosts(&name) {
                if self.secrets.grant(service, &name, host).await?.is_none() {
                    allowed = false;
                    break;
                }
            }
            if allowed {
                set.push(name);
            }
        }
        Ok(set)
    }

    /// Start a service explicitly stopped by its owner.
    pub async fn start(&self, service: &str) -> Result<bool, Denied> {
        Ok(self.switch(service, true).await?)
    }

    /// Refuses `manifest` when it needs a syscall this host does not serve.
    fn served(&self, manifest: &Manifest) -> Result<(), Denied> {
        let unserved: Vec<&str> = manifest
            .imports
            .iter()
            .map(String::as_str)
            .filter(|syscall| !self.berm.serves(syscall))
            .collect();
        if !unserved.is_empty() {
            return Err(deny(
                StatusCode::UNPROCESSABLE_ENTITY,
                format!("this host does not serve {}", unserved.join(", ")),
            )
            .explained(MISSING_HOST_CALL));
        }
        Ok(())
    }

    async fn cron_tools(&self, manifest: &Manifest) -> Result<Vec<String>, Denied> {
        let tools: Vec<String> = manifest
            .tools
            .iter()
            .filter(|tool| tool.cron)
            .map(|tool| tool.name.clone())
            .collect();
        for (tool, expression) in &manifest.crons {
            if !tools.contains(tool) {
                return Err(deny(
                    StatusCode::BAD_REQUEST,
                    format!("crons names {tool}, which is not a cron tool"),
                ));
            }
            cron::parse(expression)
                .map_err(|error| deny(StatusCode::BAD_REQUEST, format!("{tool}: {error:#}")))?;
        }
        Ok(tools)
    }

    /// How `manifest` breaks a public tool or event of `current`, the service
    /// `service` runs, that a caller used, with a call that was done, in the
    /// last [`usage::WINDOW`] days, with each caller and its last call.
    async fn broken(
        &self,
        service: &str,
        current: &Service,
        manifest: &Manifest,
    ) -> Result<Vec<Broken>, Denied> {
        let (scope, name) = host::split(service);
        let mut breaks = Vec::new();
        for broken in current.manifest.breaks(manifest) {
            let of = match &broken.surface {
                Surface::Tool(tool) => (Kind::Tool, tool.as_str(), Outcome::Done),
                Surface::Workflow(workflow) => (Kind::Workflow, workflow.as_str(), Outcome::Done),
                Surface::Event(event) => (Kind::Event, event.as_str(), Outcome::Done),
            };
            let callers = usage::read(
                &self.store,
                scope,
                name,
                Some(of),
                usage::window(usage::seconds(&self.now)),
                self.usage.unsaved(service),
            )
            .await?;
            if callers.is_empty() {
                continue;
            }
            breaks.push(Broken {
                broken: broken.to_string(),
                by: callers
                    .into_iter()
                    .map(|caller| LastUse {
                        source: caller.source,
                        caller: caller.caller,
                        last: chrono::DateTime::from_timestamp(caller.last, 0)
                            .map_or_else(String::new, |at| at.format("%Y-%m-%d").to_string()),
                    })
                    .collect(),
            });
        }
        Ok(breaks)
    }
}
