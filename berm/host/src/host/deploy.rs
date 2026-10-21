//! Deploying a service.

use crate::{
    Denied, deny,
    env::{Env, Images},
    host::{self, Host},
    record::usage,
    state::service,
    wake::{cron, table},
};
use axum::http::StatusCode;
use berm::{Program, database::Database};
use berm_api::{
    Manifest, Surface,
    host::{Deploy, Kind, MAX_DATABASE_SIZE, MIN_DATABASE_SIZE, Outcome},
};
use std::{collections::BTreeMap, path::Path, sync::Arc};

impl<E: Env> Host<E> {
    /// Run the index entry `deploy` names as `service`. Refused when it
    /// needs a syscall this host does not serve, when it makes a tool public
    /// that the program it replaces did not and `public` does not confirm it,
    /// or, unless `force`, when it breaks a public tool or event of the program
    /// it replaces that a caller used, with a call that was done, in the last
    /// [`usage::WINDOW`] days. Refused too when its `size` is outside
    /// [`MIN_DATABASE_SIZE`]..=[`MAX_DATABASE_SIZE`] or below what its
    /// database holds. A refused deploy leaves the database held to the size
    /// it was.
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
    ) -> Result<Arc<Program>, Denied> {
        let Deploy {
            reference,
            digest,
            public,
            force,
            crons,
            size,
        } = deploy;
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
            .pull(&reference, &digest)
            .await
            .map_err(|e| deny(StatusCode::BAD_GATEWAY, format!("{e:#}")))?;
        let manifest = Manifest::from_image(&image)
            .map_err(|e| deny(StatusCode::BAD_REQUEST, format!("{e:#}")))?;
        self.served(&manifest)?;

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
        let current = match service::row(&self.store, scope, name).await? {
            Some(row) => Some(
                self.load(row)
                    .await
                    .map_err(|e| deny(StatusCode::BAD_GATEWAY, format!("{e:#}")))?,
            ),
            None => None,
        };
        confirmed(current.as_deref(), &manifest, &public)?;
        let cron_tools = self
            .cron_tools(service, &manifest, crons.as_ref(), force)
            .await?;
        if let Some(current) = &current
            && !force
        {
            self.unbroken(service, current, &manifest).await?;
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
                database
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
            let program = self
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
                (self.now)() as i64,
            )
            .await?;
            Ok(program)
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
        if installed.is_ok() {
            self.waking.notify_one();
        }
        installed
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

    /// Refuses `manifest` when it breaks a public tool or event of `current`,
    /// the program `service` runs, that a caller used, with a call that was
    /// done, in the last [`usage::WINDOW`] days.
    async fn unbroken(
        &self,
        service: &str,
        current: &Program,
        manifest: &Manifest,
    ) -> Result<(), Denied> {
        let (scope, name) = host::split(service);
        let _reading = self.flushing.read().await;
        let mut used = Vec::new();
        for broken in current.manifest().breaks(manifest) {
            let of = match &broken.surface {
                Surface::Tool(tool) => (Kind::Tool, tool.as_str(), Outcome::Done),
                Surface::Event(event) => (Kind::Event, event.as_str(), Outcome::Done),
            };
            let callers = usage::read(
                &self.store,
                scope,
                name,
                Some(of),
                usage::seconds(&self.now),
                self.usage.unsaved(service),
            )
            .await?;
            if callers.is_empty() {
                continue;
            }
            let callers: Vec<String> = callers
                .iter()
                .map(|caller| {
                    let last = chrono::DateTime::from_timestamp(caller.last, 0)
                        .map_or_else(String::new, |at| at.format("%Y-%m-%d").to_string());
                    format!("{} {}, last {last}", caller.source, caller.caller)
                })
                .collect();
            used.push(format!("{broken} (used by {})", callers.join("; ")));
        }
        if !used.is_empty() {
            return Err(deny(
                StatusCode::CONFLICT,
                format!(
                    "this deploy breaks what callers used in the last {} days: {}. Deploy with `force` to override",
                    usage::WINDOW,
                    used.join(" / ")
                ),
            ));
        }
        Ok(())
    }
}

/// Refuses a deploy of `manifest` that makes a tool public that `current`
/// did not, unless `public` confirms it.
fn confirmed(
    current: Option<&Program>,
    manifest: &Manifest,
    public: &[String],
) -> Result<(), Denied> {
    let published: Vec<&str> = match current {
        Some(current) => current.manifest().publishes(manifest),
        None => manifest
            .tools
            .iter()
            .filter(|tool| tool.public)
            .map(|tool| tool.name.as_str())
            .collect(),
    };
    let unconfirmed: Vec<&str> = published
        .into_iter()
        .filter(|tool| !public.iter().any(|confirmed| confirmed == tool))
        .collect();
    if !unconfirmed.is_empty() {
        return Err(deny(
            StatusCode::CONFLICT,
            format!(
                "this deploy makes {} public: any caller may run them, with arguments of their choosing. Confirm each by listing it in `public`",
                unconfirmed.join(", ")
            ),
        ));
    }
    Ok(())
}
