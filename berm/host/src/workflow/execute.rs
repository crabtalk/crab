use crate::{
    Denied, deny,
    env::{Env, Peers},
    host::{self, Host},
    workflow::{Claim, Intent},
};
use anyhow::{Context, Result, bail, ensure};
use axum::http::StatusCode;
use berm::{
    Caller, Client, MemoryLimit, Meters, Refused, Service,
    abi::workflow::{ControlInput, ControlOutput, ControlValue},
    control,
};
use berm_api::{
    Meter, Output, digest,
    host::{Kind as UsageKind, Link, Outcome as UsageOutcome, Source as UsageSource, User},
    schema,
    workflow::{
        Version,
        graph::{Entry, Kind, Mode, Node, Producer, Selection, Source, Target},
        run::{
            Accepted, Binding, Checkpoint, Failure, Item, Prepared, PreparedCall, Run, State,
            Status, Trigger,
        },
    },
};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    error::Error as StdError,
    fmt::{Display, Formatter, Result as FmtResult},
    sync::Arc,
    time::{Duration, Instant},
};
use uuid::Uuid;

const IDEMPOTENCY_KEY: &str = "idempotency_key";

impl<E: Env> Host<E> {
    /// Resolve locally deployed dependencies without executing control functions or tools.
    pub fn workflow_bindings(&self, service: &str, workflow: &str) -> Result<Accepted> {
        let owner = self.deployed(service)?;
        let spec = owner
            .manifest
            .workflows
            .iter()
            .find(|spec| spec.entry.name == workflow)
            .context("no such workflow")?;
        let mut bindings = BTreeMap::new();
        for node in &spec.entry.graph.nodes {
            let Kind::Step { call, .. } = &node.kind else {
                continue;
            };
            let name = match &call.target {
                Target::Local { .. } => service.to_owned(),
                Target::Service {
                    service: target, ..
                } => {
                    if target.contains('/') {
                        target.clone()
                    } else {
                        host::join(host::split(service).0, target)
                    }
                }
            };
            let target = self.deployed(&name)?;
            if matches!(call.target, Target::Service { .. })
                && target.public(call.target.tool())
                && !target.reached_by_services(call.target.tool())
            {
                return Err(deny(
                    StatusCode::FORBIDDEN,
                    "workflow target takes no calls from services",
                )
                .explained(Denied::SERVICE_CALLERS)
                .into());
            }
            let binding = target.binding(call.target.clone())?;
            call.check_binding(&binding)?;
            bindings.insert(node.id.clone(), binding);
        }
        Ok(Accepted {
            bindings,
            control_budget: owner.controls[workflow],
            control_buffer: owner.manifest.buffer as u64,
            checkpoint_bytes: spec.config.checkpoint_bytes,
            deadline_ms: self.deadline.as_millis().try_into()?,
        })
    }

    pub fn prepare_workflow(
        &self,
        service: &str,
        workflow: &str,
        input: Value,
        trigger: Trigger,
    ) -> Result<Run> {
        let owner = self.deployed(service)?;
        let spec = owner
            .manifest
            .workflows
            .iter()
            .find(|spec| spec.entry.name == workflow)
            .context("no such workflow")?;
        schema::validate(&spec.entry.graph.input, &input)?;
        let created_at = (self.now)();
        Ok(Run {
            version: Version::V1,
            id: Uuid::new_v4().to_string(),
            service: service.into(),
            workflow: workflow.into(),
            source: Source {
                image: digest::prefixed(&owner.digest),
                graph: spec.entry.digest()?,
            },
            input,
            trigger,
            created_at,
            expires_at: created_at
                .checked_add(spec.config.retain.as_millis().try_into()?)
                .context("workflow retention overflow")?,
            accepted: self.workflow_bindings(service, workflow)?,
            status: Status::Pending,
            attempts: vec![],
            checkpoints: BTreeMap::new(),
            result: None,
            error: None,
        })
    }

    /// The embedder authorizes admission; this entry starts a pending, accepted run.
    pub async fn execute_workflow(&self, id: &str) -> Result<Run> {
        let run = self
            .workflows
            .get(id)
            .await?
            .context("no such workflow run")?;
        let now = (self.now)();
        let duration = Duration::from_millis(run.accepted.deadline_ms).min(self.deadline);
        let deadline = Instant::now()
            .checked_add(duration)
            .context("workflow deadline overflow")?;
        let claim = self
            .workflows
            .claim(
                id,
                Intent::Start,
                now,
                now.checked_add(duration.as_millis().try_into()?)
                    .context("workflow deadline overflow")?,
            )
            .await?;
        self.execute_attempt(run, claim, deadline).await
    }

    pub(crate) async fn execute_attempt(
        &self,
        mut run: Run,
        claim: Claim,
        deadline: Instant,
    ) -> Result<Run> {
        let mut total = Meter::default();
        // Take the claimed state, including its durable attempt identity.
        run = self
            .workflows
            .get(&run.id)
            .await?
            .context("workflow disappeared")?;
        let executed = async {
            let _slot = tokio::time::timeout_at(deadline.into(), self.gate.acquire()).await??;
            let bytes = self.workflows.image(&run.id).await?;
            let owner = self.berm.retained(&run.service, &bytes)?;
            let entry = owner
                .manifest
                .workflows
                .iter()
                .find(|spec| spec.entry.name == run.workflow)
                .context("workflow missing from retained image")?
                .entry
                .clone();

            if owner.controls[&run.workflow] != run.accepted.control_budget {
                return Err(
                    deny(StatusCode::CONFLICT, "workflow control budget changed")
                        .explained(Binding::CHANGED)
                        .into(),
                );
            }
            self.workflow_available(&run.service)?;
            for node in &entry.graph.nodes {
                if matches!(
                    run.checkpoints[&node.id].state,
                    State::Succeeded { .. } | State::Skipped
                ) {
                    continue;
                }
                ensure!(
                    Instant::now() < deadline,
                    "workflow attempt exceeded its deadline"
                );
                self.workflow_available(&run.service)?;
                if Self::skip_node(&run, node)? {
                    let mut checkpoint = run.checkpoints[&node.id].clone();
                    checkpoint.state = State::Skipped;
                    self.save_node(&claim, &mut run, node, checkpoint).await?;
                    continue;
                }
                if let Err(error) = self
                    .execute_node(&owner, &entry, &claim, &mut run, node, deadline, &mut total)
                    .await
                {
                    if !error.is::<Persistence>()
                        && !matches!(
                            run.checkpoints[&node.id].state,
                            State::Failed { .. } | State::Unknown { .. }
                        )
                    {
                        let mut checkpoint = run.checkpoints[&node.id].clone();
                        checkpoint.state = State::Failed {
                            error: Self::execution_failure(&error),
                        };
                        self.save_node(&claim, &mut run, node, checkpoint).await?;
                    }
                    return Err(error);
                }
            }
            let output = entry
                .graph
                .nodes
                .iter()
                .find(|node| matches!(node.kind, Kind::Output))
                .context("workflow has no output")?;
            match &run.checkpoints[&output.id].state {
                State::Succeeded { .. } => Ok(run.checkpoints[&output.id].clone()),
                _ => bail!("workflow output did not complete"),
            }
        }
        .await;
        match executed {
            Ok(value) => {
                self.workflows
                    .finish(
                        &claim,
                        Status::Succeeded,
                        Some(value),
                        None,
                        total,
                        (self.now)(),
                    )
                    .await?
            }
            Err(error) => {
                // A failed write leaves its preceding dispatch record intact.
                if error.is::<Persistence>() {
                    return Err(error);
                }
                let failure = error
                    .downcast_ref::<Failed>()
                    .map(|error| error.0.clone())
                    .unwrap_or_else(|| Self::execution_failure(&error));
                let unknown = run.checkpoints.values().any(|checkpoint| {
                    matches!(checkpoint.state, State::Running | State::Unknown { .. })
                });
                let status = if unknown {
                    Status::Indeterminate
                } else {
                    Status::Failed
                };
                self.workflows
                    .finish(&claim, status, None, Some(failure), total, (self.now)())
                    .await?;
            }
        }
        self.workflows
            .get(&run.id)
            .await?
            .context("workflow disappeared")
    }

    fn workflow_available(&self, service: &str) -> Result<()> {
        self.deployed(service)?;
        if self.stopped(service) || self.holds.holds(host::split(service).0) {
            return Err(deny(
                StatusCode::SERVICE_UNAVAILABLE,
                "workflow service is stopped or held",
            )
            .explained(Denied::PAUSED)
            .into());
        }
        Ok(())
    }

    fn skip_node(run: &Run, node: &Node) -> Result<bool> {
        for dependency in &node.dependencies {
            if let Producer::Node { id } = &dependency.source {
                let state = &run.checkpoints[id].state;
                ensure!(
                    matches!(state, State::Succeeded { .. } | State::Skipped),
                    "workflow dependency {id} has not completed"
                );
                if !dependency.optional && matches!(state, State::Skipped) {
                    return Ok(true);
                }
            }
        }
        for guard in &node.guards {
            let checkpoint = &run.checkpoints[&guard.producer];
            let selected = match &guard.selection {
                Selection::Present => matches!(checkpoint.state, State::Succeeded { .. }),
                Selection::Case { name } => match &checkpoint.state {
                    State::Succeeded { value } => {
                        value.as_str() == Some(name)
                            || value.as_object().is_some_and(|fields| {
                                fields.len() == 1 && fields.contains_key(name)
                            })
                    }
                    _ => false,
                },
            };
            if !selected {
                return Ok(true);
            }
        }
        Ok(false)
    }

    async fn execute_node(
        &self,
        owner: &Arc<Service>,
        entry: &Entry,
        claim: &Claim,
        run: &mut Run,
        node: &Node,
        deadline: Instant,
        total: &mut Meter,
    ) -> Result<()> {
        let mut checkpoint = run.checkpoints[&node.id].clone();
        if checkpoint.prepared.is_none() {
            let input = ControlInput {
                input: run.input.clone(),
                nodes: node
                    .dependencies
                    .iter()
                    .filter_map(|dependency| match &dependency.source {
                        Producer::Input => None,
                        Producer::Node { id } => Some((id, &run.checkpoints[id])),
                    })
                    .filter_map(|(id, checkpoint)| match &checkpoint.state {
                        State::Succeeded { value } => Some((
                            id.clone(),
                            ControlValue::Present {
                                value: value.clone(),
                            },
                        )),
                        State::Skipped => Some((id.clone(), ControlValue::Skipped)),
                        _ => None,
                    })
                    .collect(),
            };
            let meters = Arc::new(Meters::default());
            let result = owner
                .control(control::Request {
                    workflow: &run.workflow,
                    node: &node.id,
                    input: &input,
                    deadline,
                    meters: meters.clone(),
                })
                .await;
            let used = self.metered(meters.take());
            checkpoint.meter += used;
            *total += used;
            let result = match result {
                Ok(value) => value,
                Err(error) => {
                    let failure = Self::execution_failure(&error);
                    checkpoint.state = State::Failed {
                        error: failure.clone(),
                    };
                    self.save_node(claim, run, node, checkpoint).await?;
                    return Err(Failed(failure).into());
                }
            };
            let prepared = (|| -> Result<()> {
                match (&node.kind, result) {
                    (Kind::Step { call, mode }, ControlOutput::Calls { inputs }) => {
                        ensure!(
                            inputs.len() <= mode.maximum() as usize
                                && (matches!(mode, Mode::Each { .. }) || inputs.len() == 1),
                            "workflow builder exceeded its declared call bound"
                        );
                        let binding = &run.accepted.bindings[&node.id];
                        let mut calls = Vec::with_capacity(inputs.len());
                        for input in inputs {
                            schema::validate(&call.input, &input)?;
                            schema::validate(&binding.input, &input)?;
                            let operation_key = input
                                .get(IDEMPOTENCY_KEY)
                                .and_then(Value::as_str)
                                .filter(|key| !key.is_empty())
                                .map(str::to_owned);
                            if matches!(
                                call.target,
                                Target::Service {
                                    idempotency_ms: Some(_),
                                    ..
                                }
                            ) {
                                ensure!(
                                    operation_key.is_some(),
                                    "idempotent workflow targets require a nonempty idempotency_key"
                                );
                            }
                            calls.push(PreparedCall {
                                input,
                                operation_key,
                            });
                        }
                        checkpoint.items = calls
                            .iter()
                            .enumerate()
                            .map(|(index, _)| Item {
                                index: index as u32,
                                state: State::Pending,
                                dispatches: vec![],
                            })
                            .collect();
                        checkpoint.prepared = Some(Prepared::Calls { calls });
                        checkpoint.state = State::Prepared;
                    }
                    (
                        Kind::Step {
                            mode: Mode::Optional,
                            ..
                        },
                        ControlOutput::Skipped,
                    ) => checkpoint.state = State::Skipped,
                    (Kind::Branch { cases }, ControlOutput::Branch { case, value }) => {
                        let selected = cases
                            .iter()
                            .find(|candidate| candidate.name == case)
                            .context("control selected an undeclared branch")?;
                        let payload = value
                            .get(&case)
                            .context("branch result does not match its selected case")?;
                        ensure!(
                            value.as_object().is_some_and(|fields| fields.len() == 1),
                            "branch result has multiple cases"
                        );
                        schema::validate(&selected.schema, payload)?;
                        checkpoint.prepared = Some(Prepared::Branch {
                            case,
                            payload: value.clone(),
                        });
                        checkpoint.state = State::Succeeded { value };
                    }
                    (Kind::Output, ControlOutput::Output { value }) => {
                        schema::validate(&entry.graph.output, &value)?;
                        checkpoint.prepared = Some(Prepared::Output {
                            value: value.clone(),
                        });
                        checkpoint.state = State::Succeeded { value };
                    }
                    _ => bail!("control result does not match its declared node kind"),
                }
                Ok(())
            })();
            if let Err(error) = prepared {
                let failure = Self::execution_failure(&error);
                checkpoint.state = State::Failed {
                    error: failure.clone(),
                };
                self.save_node(claim, run, node, checkpoint).await?;
                return Err(Failed(failure).into());
            }
            if matches!(node.kind, Kind::Output) {
                run.checkpoints.insert(node.id.clone(), checkpoint);
                return Ok(());
            }
            self.save_node(claim, run, node, checkpoint.clone()).await?;
        }
        let Some(Prepared::Calls { calls }) = checkpoint.prepared.clone() else {
            return Ok(());
        };
        let Kind::Step { mode, call } = &node.kind else {
            bail!("call checkpoint belongs to a non-step")
        };
        for (index, prepared) in calls.iter().enumerate() {
            if matches!(checkpoint.items[index].state, State::Succeeded { .. }) {
                continue;
            }
            self.workflow_available(&run.service)?;
            ensure!(
                Instant::now() < deadline,
                "workflow attempt exceeded its deadline"
            );
            let binding = &run.accepted.bindings[&node.id];
            let target = match &call.target {
                Target::Local { .. } => Some(owner.clone()),
                Target::Service { .. }
                    if host::split(&binding.service).0 == host::split(&run.service).0 =>
                {
                    Some(self.deployed(&binding.service)?)
                }
                Target::Service { .. } => None,
            };
            // The captured target remains fixed across deployment changes.
            if let Some(target) = &target {
                if target.binding(call.target.clone()).map_err(|error| {
                    deny(StatusCode::CONFLICT, format!("{error:#}")).explained(Binding::CHANGED)
                })? != *binding
                {
                    return Err(
                        deny(StatusCode::CONFLICT, "workflow dependency binding changed")
                            .explained(Binding::CHANGED)
                            .into(),
                    );
                }
            }
            let dispatched = self
                .workflows
                .begin_dispatch(claim, &node.id, index, (self.now)())
                .await
                .map_err(Persistence)?;
            checkpoint.items[index].state = State::Running;
            checkpoint.items[index].dispatches.push(dispatched);
            checkpoint.state = State::Running;
            run.checkpoints.insert(node.id.clone(), checkpoint.clone());
            let meters = Arc::new(Meters::default());
            let identity = match &run.trigger {
                Trigger::Manual { caller } => Some(caller),
                Trigger::Schedule { .. } => None,
            };
            let user = identity
                .and_then(|caller| caller.user.as_ref())
                .map(|user| User {
                    issuer: user.issuer.clone(),
                    subject: user.subject.clone(),
                });
            let args = serde_json::to_vec(&prepared.input)?;
            let local = target.is_some();
            let result = if let Some(target) = target {
                let link = berm::Link {
                    caller: Some(Caller::Service(run.service.as_str().into())),
                    key: identity
                        .and_then(|caller| caller.key.as_deref())
                        .map(Into::into),
                    user: user.as_ref().map(host::user),
                    client: Client::default(),
                    depth: 1,
                    query: false,
                    deadline,
                    stack: Arc::from([
                        Arc::from(run.service.as_str()),
                        Arc::from(binding.service.as_str()),
                    ]),
                };
                let outcome = target
                    .link(link, call.target.tool(), args, meters.clone())
                    .await;
                match outcome {
                    Ok(outcome) => self
                        .commit_outcome(outcome)
                        .await
                        .map(|ended| (ended.result, ended.denied)),
                    Err(error) => Err(error),
                }
            } else {
                let link = Link {
                    accepted: Some(binding.clone()),
                    caller: Some(run.service.clone()),
                    from: run.service.clone(),
                    key: identity.and_then(|caller| caller.key.clone()),
                    user,
                    client: None,
                    client_ip: None,
                    depth: 1,
                    query: false,
                    timeout: deadline
                        .saturating_duration_since(Instant::now())
                        .as_millis()
                        .try_into()?,
                    stack: vec![run.service.clone(), binding.service.clone()],
                };
                self.peers
                    .call(&binding.service, call.target.tool(), args, link)
                    .await
                    .map(|reply| {
                        for (service, tools) in reply.meters {
                            for (tool, used) in tools {
                                meters.add(service.as_str().into(), tool.as_str().into(), used);
                            }
                        }
                        (reply.result, reply.denied)
                    })
            };
            let metered = meters.take();
            let used = if local {
                let outcome = match &result {
                    Ok((output, denied)) => UsageOutcome::of(output, *denied),
                    Err(_) => UsageOutcome::Failed,
                };
                self.usage.record(
                    &binding.service,
                    UsageKind::Tool,
                    call.target.tool(),
                    UsageSource::Service,
                    &run.service,
                    outcome,
                );
                self.metered(metered)
            } else {
                metered
                    .into_values()
                    .fold(Meter::default(), |mut total, used| {
                        total += used;
                        total
                    })
            };
            *total += used;
            checkpoint.meter += used;
            let state = match result {
                Ok((Output::Done(text), _)) => match serde_json::from_str(&text)
                    .map_err(anyhow::Error::from)
                    .and_then(|value| {
                        schema::validate(&call.output, &value)?;
                        Ok(value)
                    }) {
                    Ok(value) => State::Succeeded { value },
                    Err(error) => State::Unknown {
                        error: Self::execution_failure(&error),
                    },
                },
                Ok((Output::Failed(message), denied)) => State::Failed {
                    error: Failure {
                        status: if denied { 403 } else { 422 },
                        headers: BTreeMap::new(),
                        body: serde_json::from_str::<Value>(&message)
                            .ok()
                            .filter(|body| body.get("error").is_some())
                            .unwrap_or_else(|| json!({"error":message})),
                    },
                },
                Err(error) => {
                    let refused = error.is::<Refused>()
                        || error
                            .downcast_ref::<Failure>()
                            .is_some_and(|failure| (400..500).contains(&failure.status))
                        || error
                            .downcast_ref::<Denied>()
                            .is_some_and(|denied| denied.status.is_client_error());
                    let error = Self::execution_failure(&error);
                    if refused {
                        State::Failed { error }
                    } else {
                        State::Unknown { error }
                    }
                }
            };
            let dispatch = checkpoint.items[index].dispatches.last_mut().unwrap();
            dispatch.state = state.clone();
            dispatch.finished_at = Some((self.now)());
            dispatch.meter = used;
            checkpoint.items[index].state = state.clone();
            if let State::Failed { error } | State::Unknown { error } = &state {
                checkpoint.state = state.clone();
                let error = error.clone();
                self.save_node(claim, run, node, checkpoint).await?;
                return Err(Failed(error).into());
            }
            self.save_node(claim, run, node, checkpoint.clone()).await?;
        }
        let values = checkpoint
            .items
            .iter()
            .map(|item| match &item.state {
                State::Succeeded { value } => Ok(value.clone()),
                _ => bail!("workflow item did not finish"),
            })
            .collect::<Result<Vec<_>>>()?;
        let value = if matches!(mode, Mode::Each { .. }) {
            Value::Array(values)
        } else {
            values
                .into_iter()
                .next()
                .context("workflow step has no result")?
        };
        checkpoint.state = State::Succeeded { value };
        self.save_node(claim, run, node, checkpoint).await
    }

    async fn save_node(
        &self,
        claim: &Claim,
        run: &mut Run,
        node: &Node,
        checkpoint: Checkpoint,
    ) -> Result<()> {
        self.workflows
            .checkpoint(claim, &node.id, checkpoint.clone())
            .await
            .map_err(Persistence)?;
        run.checkpoints.insert(node.id.clone(), checkpoint);
        Ok(())
    }

    fn execution_failure(error: &anyhow::Error) -> Failure {
        if error.is::<MemoryLimit>() {
            return Failure::from(
                &deny(StatusCode::TOO_MANY_REQUESTS, error.to_string()).explained(Denied::MEMORY),
            );
        }
        if let Some(failure) = error.downcast_ref::<Failure>() {
            return failure.clone();
        }
        if let Some(denied) = error.downcast_ref::<Denied>() {
            return Failure::from(denied);
        }
        let (status, body) = if let Some(refused) = error.downcast_ref::<Refused>() {
            (403, json!({"error":refused.0}))
        } else {
            let mut body = json!({"error":format!("{error:#}")});
            if let Some(explain) = control::Effect::explanation(error) {
                body["explain"] = json!(explain);
            }
            (422, body)
        };
        Failure {
            status,
            headers: BTreeMap::new(),
            body,
        }
    }
}

#[derive(Debug)]
struct Failed(Failure);
impl Display for Failed {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        self.0.body.fmt(f)
    }
}
impl StdError for Failed {}
#[derive(Debug)]
struct Persistence(anyhow::Error);
impl Display for Persistence {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        self.0.fmt(f)
    }
}
impl StdError for Persistence {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        self.0.source()
    }
}
