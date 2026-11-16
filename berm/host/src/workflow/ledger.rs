use crate::{Store, host};
use anyhow::{Context, Result, ensure};
use berm_api::{
    Manifest, Meter, digest,
    workflow::{
        graph::{Entry, Kind, Target},
        run::{Attempt, Checkpoint, Completion, Dispatch, Failure, Prepared, Run, State, Status},
    },
};
use rusqlite::{Connection, OptionalExtension, Transaction, TransactionBehavior, params};
use std::{fmt, path::Path};
use uuid::Uuid;

const MIGRATIONS: &[&str] = &[
    include_str!("migrations/0001.sql"),
    include_str!("migrations/0002.sql"),
];

#[derive(Clone)]
pub struct Ledger {
    pub store: Store,
}

/// Storage already granted to this host for this owning scope.
pub struct StorageGrant {
    pub scope: String,
    pub bytes: u64,
}

#[derive(Debug, Clone)]
pub struct Claim {
    pub run: String,
    pub token: String,
    pub attempt: String,
    pub deadline_at: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Intent {
    Start,
    Continue,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Eligibility {
    Available,
    Busy,
    Expired,
    Removed,
    Stopped,
    NotFailed,
}

#[derive(Debug)]
pub enum Refused {
    Capacity,
    Missing,
    Busy,
    Expired,
    Removed,
    Ineligible,
    Invalid(String),
}

impl fmt::Display for Refused {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Capacity => f.write_str(
                "workflow checkpoint or retained image capacity exceeds the storage grant",
            ),
            Self::Missing => f.write_str("workflow run was not found"),
            Self::Busy => f.write_str("workflow run is claimed by another attempt"),
            Self::Expired => f.write_str("workflow checkpoints have expired"),
            Self::Removed => f.write_str("workflow service was deleted"),
            Self::Ineligible => f.write_str("workflow run is not eligible for this attempt"),
            Self::Invalid(message) => f.write_str(message),
        }
    }
}
impl std::error::Error for Refused {}

struct Record {
    run: Run,
    claim: Option<String>,
    lease: Option<u64>,
    invalidated: bool,
    entry: Entry,
}

impl Ledger {
    pub fn open(path: &Path) -> Result<Self> {
        Ok(Self {
            store: Store::durable(path, MIGRATIONS)?,
        })
    }

    async fn transaction<T: Send + 'static>(
        &self,
        work: impl FnOnce(&Transaction<'_>) -> Result<T> + Send + 'static,
    ) -> Result<T> {
        self.store
            .run(move |conn| {
                let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
                let result = work(&tx);
                if result.is_ok() {
                    tx.commit()?;
                }
                Ok(result)
            })
            .await?
    }

    pub async fn insert(&self, mut run: Run, image: Vec<u8>, grant: StorageGrant) -> Result<()> {
        ensure!(
            run.status == Status::Pending && run.attempts.is_empty(),
            "new workflow runs must be pending without attempts"
        );
        ensure!(
            !run.id.is_empty() && run.created_at < run.expires_at,
            "invalid workflow run identity or retention"
        );
        ensure!(
            digest::of(&image) == run.source.image,
            "workflow image digest does not match"
        );
        let manifest = Manifest::try_from(image.as_slice())?;
        let spec = manifest
            .workflows
            .iter()
            .find(|spec| spec.entry.name == run.workflow)
            .context("workflow is absent from the retained image")?;
        ensure!(
            spec.entry.digest()? == run.source.graph,
            "workflow graph digest does not match"
        );
        ensure!(
            run.accepted.checkpoint_bytes == spec.config.checkpoint_bytes,
            "checkpoint capacity differs from the image"
        );
        ensure!(
            run.accepted.control_buffer == manifest.buffer as u64,
            "control buffer differs from the image"
        );
        ensure!(
            spec.config.control_budget(run.accepted.control_budget) == run.accepted.control_budget,
            "control budget differs from the image"
        );
        ensure!(
            grant.scope == host::split(&run.service).0,
            "storage grant belongs to another scope"
        );
        berm_api::schema::validate(&spec.entry.graph.input, &run.input)?;
        ensure!(
            run.expires_at
                == run
                    .created_at
                    .checked_add(u64::try_from(spec.config.retain.as_millis())?)
                    .context("retention overflow")?,
            "retention differs from the image"
        );
        ensure!(
            run.accepted.deadline_ms > 0 && run.result.is_none() && run.error.is_none(),
            "invalid new run state"
        );
        let mut steps = 0;
        for node in &spec.entry.graph.nodes {
            if let Kind::Step { call, .. } = &node.kind {
                steps += 1;
                let binding = run
                    .accepted
                    .bindings
                    .get(&node.id)
                    .context("missing accepted dependency binding")?;
                call.check_binding(binding)?;
                if matches!(call.target, Target::Local { .. }) {
                    ensure!(
                        binding.service == run.service && binding.image == run.source.image,
                        "local binding differs from the workflow image"
                    );
                }
            }
        }
        ensure!(
            steps == run.accepted.bindings.len(),
            "accepted bindings include unknown nodes"
        );
        ensure!(
            run.checkpoints.is_empty(),
            "new workflow runs cannot supply checkpoints"
        );
        for node in &spec.entry.graph.nodes {
            run.checkpoints.insert(
                node.id.clone(),
                Checkpoint {
                    state: State::Pending,
                    prepared: None,
                    items: Vec::new(),
                    meter: Meter::default(),
                },
            );
        }
        let entry = spec.entry.clone();
        let body = Self::encode(&run)?;
        self.transaction(move |tx| {
            ensure!(Self::load(tx, &run.id)?.is_none(), "workflow run identity already exists");
            let mut needed = run.accepted.checkpoint_bytes;
            let retained: bool = tx.query_row(
                "SELECT EXISTS(SELECT 1 FROM runs WHERE scope=?1 AND image=?2)",
                params![grant.scope, run.source.image], |row| row.get(0),
            )?;
            if !retained { needed = needed.checked_add(image.len() as u64).ok_or(Refused::Capacity)?; }
            let reserved = Self::allocation(tx, &grant.scope)?;
            if needed > grant.bytes.saturating_sub(reserved) { return Err(Refused::Capacity.into()); }
            tx.execute("INSERT OR IGNORE INTO images(digest,image,bytes) VALUES (?1,?2,?3)", params![run.source.image, image, i64::try_from(image.len())?])?;
            tx.execute("INSERT INTO runs(id,scope,service,workflow,image,capacity,expires_at,body,entry) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
                params![run.id, grant.scope, run.service, run.workflow, run.source.image, i64::try_from(run.accepted.checkpoint_bytes)?, i64::try_from(run.expires_at)?, body, serde_json::to_vec(&entry)?])?;
            Ok(())
        }).await
    }

    pub async fn get(&self, id: &str) -> Result<Option<Run>> {
        let id = id.to_owned();
        self.store
            .read(move |conn| {
                Ok(Self::load(conn, &id).map(|record| record.map(|record| record.run)))
            })
            .await?
    }

    pub async fn image(&self, id: &str) -> Result<Vec<u8>> {
        let id = id.to_owned();
        self.store
            .read(move |conn| {
                conn.query_row(
                    "SELECT i.image FROM images i JOIN runs r ON r.image=i.digest WHERE r.id=?1",
                    [id],
                    |row| row.get(0),
                )
            })
            .await
    }

    pub async fn reserved(&self, scope: &str) -> Result<u64> {
        let scope = scope.to_owned();
        self.store
            .read(move |conn| Ok(Self::allocation(conn, &scope)))
            .await?
    }

    pub async fn checkpoint_eligibility(&self, id: &str, now: u64) -> Result<Eligibility> {
        let id = id.to_owned();
        self.store
            .read(move |conn| {
                Ok((|| {
                    let record = Self::load(conn, &id)?.ok_or(Refused::Missing)?;
                    Ok(Self::eligibility(&record, now))
                })())
            })
            .await?
    }

    pub async fn claim(&self, id: &str, intent: Intent, now: u64, deadline: u64) -> Result<Claim> {
        let id = id.to_owned();
        self.transaction(move |tx| {
            ensure!(now < deadline, "attempt deadline must be in the future");
            let mut record = Self::load(tx, &id)?.ok_or(Refused::Missing)?;
            if record.invalidated {
                return Err(Refused::Removed.into());
            }
            if now >= record.run.expires_at {
                return Err(Refused::Expired.into());
            }
            if record.lease.is_some_and(|lease| lease > now) {
                return Err(Refused::Busy.into());
            }
            if record.claim.is_some() {
                Self::interrupted(&mut record, now);
                Self::completion(tx, &record.run)?;
            }
            let allowed = match intent {
                Intent::Start => record.run.status == Status::Pending,
                Intent::Continue => {
                    matches!(record.run.status, Status::Failed | Status::Indeterminate)
                }
            };
            if !allowed {
                return Err(Refused::Ineligible.into());
            }
            let claim = Claim {
                run: id.clone(),
                token: Uuid::new_v4().to_string(),
                attempt: Uuid::new_v4().to_string(),
                deadline_at: deadline,
            };
            record.run.status = Status::Running;
            record.run.error = None;
            record.run.attempts.push(Attempt {
                id: claim.attempt.clone(),
                started_at: now,
                deadline_at: deadline,
                finished_at: None,
                status: Status::Running,
                meter: Meter::default(),
                error: None,
            });
            tx.execute(
                "UPDATE runs SET body=?2,claim=?3,lease=?4 WHERE id=?1",
                params![
                    id,
                    Self::encode(&record.run)?,
                    claim.token,
                    i64::try_from(deadline)?
                ],
            )?;
            Ok(claim)
        })
        .await
    }

    pub async fn checkpoint(
        &self,
        claim: &Claim,
        node: &str,
        checkpoint: Checkpoint,
    ) -> Result<()> {
        let node = node.to_owned();
        self.write(claim.clone(), move |record| {
            let current = record
                .run
                .checkpoints
                .get_mut(&node)
                .context("unknown workflow node")?;
            current
                .advance(checkpoint)
                .map_err(|message| Refused::Invalid(message))?;
            Ok(())
        })
        .await
    }

    pub async fn begin_dispatch(
        &self,
        claim: &Claim,
        node: &str,
        index: usize,
        now: u64,
    ) -> Result<Dispatch> {
        let (claim, node) = (claim.clone(), node.to_owned());
        self.transaction(move |tx| {
            let mut record = Self::fenced(tx, &claim)?;
            ensure!(
                record.lease.is_some_and(|lease| lease > now),
                "workflow dispatch deadline expired"
            );
            ensure!(
                !record.invalidated && record.run.status == Status::Running,
                "workflow cannot dispatch in its current state"
            );
            let checkpoint = record
                .run
                .checkpoints
                .get_mut(&node)
                .context("unknown workflow node")?;
            let Some(Prepared::Calls { calls }) = &checkpoint.prepared else {
                anyhow::bail!("workflow calls are not prepared")
            };
            ensure!(
                index < calls.len(),
                "workflow item is outside its prepared list"
            );
            let item = checkpoint
                .items
                .get_mut(index)
                .context("workflow item is missing")?;
            ensure!(
                !matches!(
                    item.state,
                    State::Running | State::Succeeded { .. } | State::Skipped
                ),
                "workflow item cannot dispatch in its current state"
            );
            let dispatch = Dispatch {
                id: Uuid::new_v4().to_string(),
                attempt: claim.attempt.clone(),
                started_at: now,
                finished_at: None,
                state: State::Running,
                meter: Meter::default(),
            };
            item.state = State::Running;
            item.dispatches.push(dispatch.clone());
            checkpoint.state = State::Running;
            tx.execute(
                "UPDATE runs SET body=?2 WHERE id=?1",
                params![claim.run, Self::encode(&record.run)?],
            )?;
            Ok(dispatch)
        })
        .await
    }

    pub async fn finish(
        &self,
        claim: &Claim,
        status: Status,
        output: Option<Checkpoint>,
        error: Option<Failure>,
        meter: Meter,
        now: u64,
    ) -> Result<()> {
        ensure!(
            matches!(
                status,
                Status::Succeeded | Status::Failed | Status::Indeterminate | Status::Cancelled
            ),
            "attempt must finish in a terminal state"
        );
        let claim = claim.clone();
        self.transaction(move |tx| {
            let mut record = Self::fenced(tx, &claim)?;
            let result = match output {
                Some(checkpoint) => {
                    ensure!(status == Status::Succeeded, "only success may record workflow output");
                    let output = record.entry.graph.nodes.iter().find(|node| matches!(node.kind, Kind::Output)).context("workflow has no output")?;
                    let State::Succeeded {value} = &checkpoint.state else { anyhow::bail!("workflow output did not succeed") };
                    ensure!(matches!(&checkpoint.prepared,Some(Prepared::Output {value:prepared}) if prepared == value), "workflow result differs from its output checkpoint");
                    berm_api::schema::validate(&record.entry.graph.output,value)?;
                    let result = value.clone();
                    record.run.checkpoints.get_mut(&output.id).context("output checkpoint is missing")?.advance(checkpoint).map_err(Refused::Invalid)?;
                    ensure!(record.run.checkpoints.values().all(|checkpoint| matches!(checkpoint.state, State::Succeeded { .. } | State::Skipped)), "success requires all nodes to finish");
                    Some(result)
                },
                None => {
                    ensure!(status != Status::Succeeded,"success requires a complete output checkpoint");
                    None
                },
            };
            record.run.status = status;
            record.run.result = result;
            record.run.error = error.clone();
            let attempt = record.run.attempts.last_mut().context("attempt is missing")?;
            ensure!(attempt.id == claim.attempt, "attempt identity changed");
            attempt.status = status;
            attempt.finished_at = Some(now);
            attempt.error = error;
            attempt.meter = meter;
            tx.execute("UPDATE runs SET body=?2,claim=NULL,lease=NULL WHERE id=?1", params![claim.run, Self::encode(&record.run)?])?;
            Self::completion(tx, &record.run)?;
            Ok(())
        }).await
    }

    pub async fn pending_completions(&self, limit: u32) -> Result<Vec<Completion>> {
        let rows = self
            .store
            .run(move |conn| {
                let mut query =
                    conn.prepare("SELECT body FROM completions ORDER BY rowid LIMIT ?1")?;
                query
                    .query_map([limit], |row| row.get::<_, Vec<u8>>(0))?
                    .collect::<rusqlite::Result<Vec<_>>>()
            })
            .await?;
        rows.into_iter()
            .map(|body| Ok(serde_json::from_slice(&body)?))
            .collect()
    }

    pub async fn acknowledge_completion(&self, run: &str, attempt: &str) -> Result<()> {
        let (run, attempt) = (run.to_owned(), attempt.to_owned());
        self.store
            .run(move |conn| {
                conn.execute(
                    "DELETE FROM completions WHERE run=?1 AND attempt=?2",
                    params![run, attempt],
                )
            })
            .await?;
        Ok(())
    }

    fn completion(tx: &Transaction<'_>, run: &Run) -> Result<()> {
        let completion = Completion::try_from(run).map_err(Refused::Invalid)?;
        tx.execute("INSERT INTO completions(run,attempt,body) VALUES(?1,?2,?3) ON CONFLICT(run,attempt) DO NOTHING",params![completion.run,completion.attempt,serde_json::to_vec(&completion)?])?;
        Ok(())
    }

    async fn write(
        &self,
        claim: Claim,
        work: impl FnOnce(&mut Record) -> Result<()> + Send + 'static,
    ) -> Result<()> {
        self.transaction(move |tx| {
            let mut record = Self::fenced(tx, &claim)?;
            work(&mut record)?;
            tx.execute(
                "UPDATE runs SET body=?2 WHERE id=?1",
                params![claim.run, Self::encode(&record.run)?],
            )?;
            Ok(())
        })
        .await
    }

    pub async fn recover(&self, now: u64) -> Result<usize> {
        self.transaction(move |tx| Self::recover_in(tx, now)).await
    }

    fn recover_in(tx: &Transaction<'_>, now: u64) -> Result<usize> {
        let mut query = tx.prepare("SELECT id FROM runs WHERE claim IS NOT NULL AND lease<=?1")?;
        let ids = query
            .query_map([i64::try_from(now)?], |row| row.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        for id in &ids {
            let mut record = Self::load(tx, id)?.context("run disappeared during recovery")?;
            Self::interrupted(&mut record, now);
            tx.execute(
                "UPDATE runs SET body=?2,claim=NULL,lease=NULL WHERE id=?1",
                params![id, Self::encode(&record.run)?],
            )?;
            Self::completion(tx, &record.run)?;
        }
        Ok(ids.len())
    }

    pub async fn invalidate(&self, service: &str) -> Result<()> {
        let service = service.to_owned();
        self.transaction(move |tx| {
            tx.execute("UPDATE runs SET invalidated=1 WHERE service=?1", [service])?;
            Ok(())
        })
        .await
    }

    pub async fn delete(&self, id: &str, now: u64) -> Result<bool> {
        let id = id.to_owned();
        self.transaction(move |tx| {
            Self::recover_in(tx, now)?;
            let Some(record) = Self::load(tx, &id)? else {
                return Ok(false);
            };
            if record.lease.is_some_and(|lease| lease > now) {
                return Err(Refused::Busy.into());
            }
            tx.execute("DELETE FROM runs WHERE id=?1", [id])?;
            Self::collect_images(tx)?;
            Ok(true)
        })
        .await
    }

    pub async fn expire(&self, now: u64) -> Result<usize> {
        self.transaction(move |tx| {
            Self::recover_in(tx, now)?;
            let deleted = tx.execute(
                "DELETE FROM runs WHERE expires_at<=?1 AND (lease IS NULL OR lease<=?1)",
                [i64::try_from(now)?],
            )?;
            Self::collect_images(tx)?;
            Ok(deleted)
        })
        .await
    }

    fn eligibility(record: &Record, now: u64) -> Eligibility {
        if record.invalidated {
            Eligibility::Removed
        } else if now >= record.run.expires_at {
            Eligibility::Expired
        } else if record.lease.is_some_and(|lease| lease > now) {
            Eligibility::Busy
        } else if matches!(record.run.status, Status::Failed | Status::Indeterminate)
            || record.claim.is_some()
        {
            Eligibility::Available
        } else {
            Eligibility::NotFailed
        }
    }

    fn fenced(conn: &Connection, claim: &Claim) -> Result<Record> {
        let record = Self::load(conn, &claim.run)?.ok_or(Refused::Missing)?;
        if record.claim.as_deref() != Some(&claim.token) {
            return Err(Refused::Busy.into());
        }
        ensure!(
            record
                .run
                .attempts
                .last()
                .is_some_and(|attempt| attempt.id == claim.attempt),
            "attempt identity changed"
        );
        Ok(record)
    }

    fn interrupted(record: &mut Record, now: u64) {
        record.run.interrupt(Self::interruption(), now);
    }

    fn interruption() -> Failure {
        Failure {
            status: 503,
            headers: Default::default(),
            body: serde_json::json!({"error":"execution stopped before all outcomes were checkpointed"}),
        }
    }

    fn encode(run: &Run) -> Result<Vec<u8>> {
        let body = serde_json::to_vec(run)?;
        let mut required = body.len();
        if matches!(run.status, Status::Running | Status::Cancelling) {
            let mut interrupted = run.clone();
            interrupted.interrupt(Self::interruption(), u64::MAX);
            required = required.max(serde_json::to_vec(&interrupted)?.len());
        }
        if required as u64 > run.accepted.checkpoint_bytes {
            return Err(Refused::Capacity.into());
        }
        Ok(body)
    }

    fn load(conn: &Connection, id: &str) -> Result<Option<Record>> {
        let row = conn
            .query_row(
                "SELECT body,claim,lease,invalidated,entry FROM runs WHERE id=?1",
                [id],
                |row| {
                    Ok((
                        row.get::<_, Vec<u8>>(0)?,
                        row.get::<_, Option<String>>(1)?,
                        row.get::<_, Option<i64>>(2)?,
                        row.get::<_, bool>(3)?,
                        row.get::<_, Vec<u8>>(4)?,
                    ))
                },
            )
            .optional()?;
        row.map(|(body, claim, lease, invalidated, entry)| {
            Ok(Record {
                run: serde_json::from_slice(&body)?,
                claim,
                lease: lease.map(u64::try_from).transpose()?,
                invalidated,
                entry: serde_json::from_slice(&entry)?,
            })
        })
        .transpose()
    }

    fn allocation(conn: &Connection, scope: &str) -> Result<u64> {
        let mut bytes = 0_u64;
        let mut query = conn.prepare("SELECT capacity FROM runs WHERE scope=?1")?;
        for capacity in query.query_map([scope], |row| row.get::<_, i64>(0))? {
            bytes = bytes
                .checked_add(u64::try_from(capacity?)?)
                .ok_or(Refused::Capacity)?;
        }
        let mut query = conn.prepare("SELECT DISTINCT i.digest,i.bytes FROM images i JOIN runs r ON r.image=i.digest WHERE r.scope=?1")?;
        for size in query.query_map([scope], |row| row.get::<_, i64>(1))? {
            bytes = bytes
                .checked_add(u64::try_from(size?)?)
                .ok_or(Refused::Capacity)?;
        }
        Ok(bytes)
    }

    fn collect_images(conn: &Connection) -> Result<()> {
        conn.execute(
            "DELETE FROM images WHERE NOT EXISTS(SELECT 1 FROM runs WHERE image=images.digest)",
            [],
        )?;
        Ok(())
    }
}
