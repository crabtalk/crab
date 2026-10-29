//! Deploying a service.

use crate::{
    Denied, deny,
    env::{Env, Images},
    host::{self, Host},
    record::{meter, usage},
    state::service,
    wake::{cron, table},
};
use axum::http::StatusCode;
use berm::{Service, database::Database};
use berm_api::{
    Manifest, Surface,
    host::{
        Breaking, Broken, Checked, Deploy, Kind, LastUse, MAX_DATABASE_SIZE, MIN_DATABASE_SIZE,
        Outcome,
    },
};
use std::{collections::BTreeMap, path::Path, sync::Arc};

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
    /// `seed`, a SQLite file on the databases' filesystem, starts the
    /// service's database, held to the image's migrations like any other:
    /// refused when the service has a database or the image keeps none. A
    /// deploy refused once the seed is in place deletes it.
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
            crons,
            size,
            stopped,
            project,
            exempt,
            vars,
        } = deploy;
        let (image, manifest) = self.image(&reference, &digest, size).await?;
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
        let current = self.current(service).await?;
        self.moving(service, project.as_deref(), force).await?;
        let cron_tools = self
            .cron_tools(service, &manifest, crons.as_ref(), force)
            .await?;
        if let Some(current) = &current
            && !force
        {
            let breaks = Broken::exempted(self.broken(service, current, &manifest).await?, &exempt);
            if !breaks.is_empty() {
                return Err(deny(StatusCode::CONFLICT, Breaking(&breaks).to_string()));
            }
        }
        if let Some(seed) = seed {
            if !manifest.keeps_database() {
                return Err(deny(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    format!("{service} keeps no database to seed"),
                ));
            }
            self.databases
                .seed(&key, seed)
                .map_err(|e| deny(StatusCode::CONFLICT, format!("{e:#}")))?;
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
            let deployed = self
                .compile(key.clone(), image)
                .await
                .map_err(|e| deny(StatusCode::BAD_REQUEST, format!("{e:#}")))?;
            service::install(
                &self.store,
                scope,
                name,
                &reference,
                &digest,
                &manifest.version,
                size,
                deprecated,
                cron_tools,
                crons,
                stopped,
                project,
                manifest.deps.vars.keys().cloned().collect(),
                vars,
                (self.now)() as i64,
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
            && installed.is_err()
            && let Err(e) = self.databases.remove(&key)
        {
            tracing::error!(service, error = %e, "the refused seed stays");
        }
        if let Ok(Installed {
            service: deployed, ..
        }) = &installed
        {
            self.waking.notify_one();
            // Stored bytes are otherwise sampled once a day, so a service
            // deployed or seeded after the day's sample would read as before.
            let today = self.today();
            if let Err(e) =
                meter::sample(&self.store, service, &self.databases, deployed.bytes, today).await
            {
                tracing::warn!(service, "sampling stored bytes failed: {e:#}");
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
        let key = service.to_owned();
        let lock = self
            .deploying
            .lock()
            .entry(key.clone())
            .or_default()
            .clone();
        let _deploying = lock.lock().await;
        let current = self.current(service).await?;
        self.moving(service, deploy.project.as_deref(), deploy.force)
            .await?;
        self.cron_tools(service, &manifest, deploy.crons.as_ref(), deploy.force)
            .await?;
        let breaks = match &current {
            Some(current) => Broken::exempted(
                self.broken(service, current, &manifest).await?,
                &deploy.exempt,
            ),
            None => Vec::new(),
        };
        let exists = self.databases.path(&key).exists();
        if seeded && !manifest.keeps_database() {
            return Err(deny(
                StatusCode::UNPROCESSABLE_ENTITY,
                format!("{service} keeps no database to seed"),
            ));
        }
        if seeded && exists {
            return Err(deny(
                StatusCode::CONFLICT,
                format!("{service} has a database, so a seed cannot start it"),
            ));
        }
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
            calls: manifest.deps.calls.clone(),
            migrations,
            breaks,
            secrets: manifest.deps.secrets.clone(),
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
        let manifest = Manifest::try_from(image.as_slice())
            .map_err(|e| deny(StatusCode::BAD_REQUEST, format!("{e:#}")))?;
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

    /// Refuses `manifest` when it needs a syscall this host does not serve.
    fn served(&self, manifest: &Manifest) -> Result<(), Denied> {
        let unserved: Vec<&str> = manifest
            .deps
            .syscalls
            .keys()
            .map(String::as_str)
            .filter(|syscall| !self.berm.serves(syscall))
            .collect();
        if !unserved.is_empty() {
            return Err(deny(
                StatusCode::UNPROCESSABLE_ENTITY,
                format!("this host does not serve {}", unserved.join(", ")),
            ));
        }
        Ok(())
    }

    /// The cron tools of `manifest`, refused when `crons` names another tool or
    /// a bad expression, or, unless `force`, undoes a cron `service`'s owner
    /// set since its last deploy.
    async fn cron_tools(
        &self,
        service: &str,
        manifest: &Manifest,
        crons: Option<&BTreeMap<String, String>>,
        force: bool,
    ) -> Result<Vec<String>, Denied> {
        let cron_tools: Vec<String> = manifest
            .tools
            .iter()
            .filter(|spec| spec.cron)
            .map(|spec| spec.name.clone())
            .collect();
        let Some(crons) = crons else {
            return Ok(cron_tools);
        };
        for (tool, expression) in crons {
            if !cron_tools.contains(tool) {
                return Err(deny(
                    StatusCode::BAD_REQUEST,
                    format!("`crons` names {tool:?}, which is not a cron tool of this image"),
                ));
            }
            cron::parse(expression)
                .map_err(|e| deny(StatusCode::BAD_REQUEST, format!("{tool}: {e:#}")))?;
        }
        if force {
            return Ok(cron_tools);
        }
        let (scope, name) = host::split(service);
        let undone: Vec<String> = table::edited(&self.store, scope, name)
            .await?
            .into_iter()
            .filter(|(tool, cron, _)| cron_tools.contains(tool) && crons.get(tool) != cron.as_ref())
            .map(|(tool, cron, _)| {
                format!(
                    "{tool}: the deploy sets {}, its owner set {}",
                    crons.get(&tool).map_or("none", String::as_str),
                    cron.as_deref().unwrap_or("none")
                )
            })
            .collect();
        if !undone.is_empty() {
            return Err(deny(
                StatusCode::CONFLICT,
                format!(
                    "this deploy undoes the crons its owner set since the last deploy: {}. Take the owner's crons into the deploy's config, or deploy with `force` to override",
                    undone.join(" / ")
                ),
            ));
        }
        Ok(cron_tools)
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
