//! Deploying a set of services together.

use crate::{
    Denied, deny,
    env::Env,
    host::{self, Host, Installed},
    state::service,
};
use axum::http::StatusCode;
use berm_api::host::{Breaking, Deploy, Plan, SetFailed, SetRefused};
use std::collections::BTreeMap;

impl<E: Env> Host<E> {
    /// Deploy `members`, services of one scope, together. Each is checked as
    /// [`Host::check`] checks it, and one refused refuses the set, naming each
    /// one's reasons. Then each switches in the [`Plan`]'s order, deployed with
    /// its exempt callers; one that fails to switch puts those switched before
    /// it back on what they ran, removing one that ran nothing. Migrations
    /// they applied stay applied.
    pub async fn deploy_set(
        &self,
        members: Vec<(String, Deploy)>,
    ) -> Result<Vec<(String, Installed)>, Denied> {
        let Some(scope) = members
            .first()
            .map(|(service, _)| host::split(service).0.to_owned())
        else {
            return Ok(Vec::new());
        };
        if members
            .iter()
            .any(|(service, _)| host::split(service).0 != scope.as_str())
        {
            return Err(deny(
                StatusCode::BAD_REQUEST,
                "a set's services are of one scope",
            ));
        }
        let mut checked = BTreeMap::new();
        let mut refused = BTreeMap::new();
        let mut status = StatusCode::CONFLICT;
        let mut explain = None;
        for (service, deploy) in &members {
            let name = host::split(service).1.to_owned();
            match self.check(service, deploy.clone(), false).await {
                Ok(check) => {
                    checked.insert(name, check);
                }
                Err(denied) => {
                    status = denied.status;
                    explain = denied.explain.or(explain);
                    refused.insert(name, denied.message);
                }
            }
        }
        if refused.is_empty() {
            let plan = Plan::new(&checked);
            for (service, deploy) in &members {
                let name = host::split(service).1;
                if !deploy.force
                    && let Some(breaks) = plan.breaks.get(name)
                    && !breaks.is_empty()
                {
                    refused.insert(name.to_owned(), Breaking(breaks).to_string());
                }
            }
            if refused.is_empty() {
                return self.install_set(&scope, members, plan).await;
            }
        }
        Err(Denied {
            explain,
            ..deny(status, SetRefused(&refused).to_string())
        })
    }

    /// Switch `members` in `plan`'s order, putting those switched back when
    /// one fails.
    async fn install_set(
        &self,
        scope: &str,
        mut members: Vec<(String, Deploy)>,
        mut plan: Plan,
    ) -> Result<Vec<(String, Installed)>, Denied> {
        let mut switched: Vec<(String, Option<Deploy>)> = Vec::new();
        let mut installed = Vec::new();
        for name in &plan.order {
            let at = members
                .iter()
                .position(|(service, _)| host::split(service).1 == name)
                .ok_or_else(|| {
                    deny(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "a planned service is not in the set",
                    )
                })?;
            let (service, mut deploy) = members.swap_remove(at);
            deploy.exempt = plan.exempt.remove(name).unwrap_or_default();
            let before = service::row(&self.store, scope, name)
                .await?
                .map(|row| Deploy {
                    reference: row.reference,
                    digest: row.digest,
                    force: true,
                    crons: None,
                    size: row.size,
                    stopped: self.stops.stopped(&service),
                    project: row.project,
                    exempt: Vec::new(),
                    vars: Default::default(),
                });
            match self.deploy(&service, deploy, None).await {
                Ok(one) => {
                    installed.push((service.clone(), one));
                    switched.push((service, before));
                }
                Err(denied) => {
                    for (service, before) in switched.into_iter().rev() {
                        let restored = match before {
                            Some(before) => self.deploy(&service, before, None).await.map(drop),
                            None => self.remove(&service).await.map(drop).map_err(|e| {
                                deny(StatusCode::INTERNAL_SERVER_ERROR, format!("{e:#}"))
                            }),
                        };
                        if let Err(e) = restored {
                            tracing::error!(service, error = %e, "a set's service was not put back");
                        }
                    }
                    return Err(Denied {
                        message: SetFailed {
                            name,
                            reason: &denied.message,
                        }
                        .to_string(),
                        ..denied
                    });
                }
            }
        }
        Ok(installed)
    }
}
