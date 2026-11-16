use crate::{
    deny,
    env::{Env, Peers},
    host::{self, Host},
    state::store::Store,
};
use anyhow::Result;
use axum::http::StatusCode;
use berm_api::{
    schedule,
    workflow::{
        config::{ScheduleInput, Spec},
        run::{Accepted, Run, Trigger},
        schedule::Occurrence,
        view::Start,
    },
};
use rusqlite::{Transaction, params};
use serde_json::Value;
use std::time::Duration;

pub(crate) struct Schedules;
impl Schedules {
    pub fn install(
        tx: &Transaction<'_>,
        scope: &str,
        name: &str,
        specs: &[Spec],
        now: i64,
    ) -> Result<()> {
        let scheduled: Vec<_> = specs
            .iter()
            .filter(|spec| spec.config.schedule.is_some())
            .collect();
        let names: Vec<_> = scheduled.iter().map(|spec| &spec.entry.name).collect();
        tx.execute(
            "DELETE FROM workflow_schedules WHERE scope=?1 AND name=?2
             AND workflow NOT IN (SELECT value FROM json_each(?3))",
            params![scope, name, serde_json::to_string(&names)?],
        )?;
        for spec in scheduled {
            let configured = spec.config.schedule.as_ref().unwrap();
            configured.check_input(&spec.entry.graph.input)?;
            tx.execute(
                "INSERT INTO workflow_schedules(scope,name,workflow,revision,cron,input,due)
                 VALUES(?1,?2,?3,?4,?5,?6,?7) ON CONFLICT(scope,name,workflow) DO UPDATE SET
                 revision=excluded.revision,cron=excluded.cron,input=excluded.input,
                 due=IIF(workflow_schedules.revision=excluded.revision,workflow_schedules.due,excluded.due)",
                params![scope,name,spec.entry.name,configured.revision(),configured.cron,
                    serde_json::to_string(&configured.input)?,schedule::next(&configured.cron,now)?],
            )?;
        }
        Ok(())
    }

    async fn advance(store: &Store, now: i64, reset: bool) -> Result<Vec<Occurrence>> {
        store
            .run(move |conn| {
                let advanced = (|| -> Result<Vec<Occurrence>> {
                    let tx = conn.transaction()?;
                    let pending = {
                        let mut statement = tx.prepare(
                            "SELECT w.scope,w.name,w.workflow,w.revision,w.cron,w.input,w.due,
                                (s.stopped=0 AND NOT EXISTS(SELECT 1 FROM held_scopes h WHERE h.scope=w.scope))
                             FROM workflow_schedules w JOIN services s USING(scope,name)
                             WHERE ?2 OR due<=?1 ORDER BY due LIMIT IIF(?2,-1,32)",
                        )?;
                        statement
                            .query_map(params![now, reset], |row| {
                                Ok((
                                    row.get::<_, String>(0)?,
                                    row.get::<_, String>(1)?,
                                    row.get::<_, String>(2)?,
                                    row.get::<_, String>(3)?,
                                    row.get::<_, String>(4)?,
                                    row.get::<_, String>(5)?,
                                    row.get::<_, i64>(6)?,
                                    row.get::<_, bool>(7)?,
                                ))
                            })?
                            .collect::<rusqlite::Result<Vec<_>>>()?
                    };
                    let mut occurrences = Vec::new();
                    for (scope, name, workflow, revision, cron, input, due, enabled) in pending {
                        tx.execute(
                            "UPDATE workflow_schedules SET due=?4 WHERE scope=?1 AND name=?2 AND workflow=?3",
                            params![scope, name, workflow, schedule::next(&cron, now)?],
                        )?;
                        if !reset && enabled && due > now.saturating_sub(Occurrence::WINDOW_MS as i64) {
                            occurrences.push(Occurrence {
                                service: host::join(&scope, &name),
                                workflow,
                                revision,
                                at: u64::try_from(due)?,
                                input: serde_json::from_str(&input)?,
                            });
                        }
                    }
                    tx.commit()?;
                    Ok(occurrences)
                })();
                Ok(advanced)
            })
            .await?
    }
}

impl<E: Env> Host<E> {
    pub fn prepare_scheduled_workflow(
        &self,
        service: &str,
        workflow: &str,
        input: Value,
        key: &str,
        at: u64,
        accepted: Accepted,
    ) -> Result<Run> {
        let offer = self.workflow_offer(service, workflow)?;
        let configured = offer.spec.config.schedule.as_ref().ok_or_else(|| {
            deny(StatusCode::CONFLICT, "workflow no longer has a schedule")
                .explained(Occurrence::STALE)
        })?;
        let occurrence = Occurrence {
            service: service.into(),
            workflow: workflow.into(),
            revision: configured.revision(),
            at,
            input: input.clone(),
        };
        if occurrence.id() != key {
            return Err(deny(StatusCode::CONFLICT, "workflow schedule changed")
                .explained(Occurrence::STALE)
                .into());
        }
        occurrence
            .check(&offer.spec, (self.now)())
            .map_err(|error| {
                deny(StatusCode::CONFLICT, error.to_string()).explained(
                    if error.is::<ScheduleInput>() {
                        ScheduleInput::EXPLAIN
                    } else {
                        Occurrence::STALE
                    },
                )
            })?;
        self.prepare_accepted_workflow(
            service,
            workflow,
            input,
            Trigger::Schedule {
                key: key.into(),
                at,
            },
            accepted,
        )
    }

    pub async fn reset_workflow_schedules(&self) -> Result<()> {
        Schedules::advance(&self.store, (self.now)() as i64, true).await?;
        Ok(())
    }

    pub async fn due_workflows(&self) -> Result<Vec<Occurrence>> {
        Schedules::advance(&self.store, (self.now)() as i64, false).await
    }

    pub async fn fire_scheduled_workflow(&self, occurrence: Occurrence) -> Result<String> {
        self.workflow_available(&occurrence.service)?;
        let offer = self.workflow_offer(&occurrence.service, &occurrence.workflow)?;
        occurrence
            .check(&offer.spec, (self.now)())
            .map_err(|error| {
                let code = if error.is::<ScheduleInput>() {
                    ScheduleInput::EXPLAIN
                } else {
                    Occurrence::STALE
                };
                deny(StatusCode::CONFLICT, error.to_string()).explained(code)
            })?;
        let id = occurrence.id();
        if self.peers.scheduled(&occurrence).await? {
            return Ok(id);
        }
        let inspection = match self.workflows.admission(&id).await? {
            Some((inspection, _)) => inspection,
            None => Self::workflow_inspection(&occurrence.service, offer, None)?,
        };
        self.start_workflow_trigger(
            &occurrence.service,
            &occurrence.workflow,
            Start {
                id: id.clone(),
                quote: inspection.quote,
                input: occurrence.input,
            },
            Trigger::Schedule {
                key: id.clone(),
                at: occurrence.at,
            },
        )
        .await?;
        Ok(id)
    }

    pub(crate) async fn workflow_timer(&self) {
        if let Err(error) = self.reset_workflow_schedules().await {
            tracing::error!(%error,"resetting workflow schedules failed");
            return;
        }
        loop {
            match self.due_workflows().await {
                Ok(occurrences) => {
                    for occurrence in occurrences {
                        let host = self.clone();
                        tokio::spawn(async move {
                            if let Err(error) = host.fire_scheduled_workflow(occurrence).await {
                                tracing::error!(%error,"scheduled workflow admission refused");
                            }
                        });
                    }
                }
                Err(error) => tracing::error!(%error,"advancing workflow schedules failed"),
            }
            let next = self
                .store
                .read(|conn| {
                    conn.query_row("SELECT MIN(due) FROM workflow_schedules", [], |row| {
                        row.get::<_, Option<i64>>(0)
                    })
                })
                .await;
            let wait = match next {
                Ok(Some(due)) => Some((due - (self.now)() as i64).max(0) as u64),
                Ok(None) => None,
                Err(error) => {
                    tracing::error!(%error,"reading workflow schedule deadline failed");
                    Some(Occurrence::WINDOW_MS)
                }
            };
            match wait {
                Some(wait) => {
                    tokio::select! {_=tokio::time::sleep(Duration::from_millis(wait))=>{},_=self.scheduling.notified()=>{}}
                }
                None => self.scheduling.notified().await,
            }
        }
    }
}
