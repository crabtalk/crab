use crate::{
    Denied, deny,
    env::{Env, Peers},
    host::{self, Host},
    workflow::{Claim, Eligibility, Intent, Refused as LedgerRefused},
};
use anyhow::{Context, Result, bail, ensure};
use axum::http::StatusCode;
use berm::{
    Caller, Client, Link as InvocationLink, MemoryLimit, Meters, Refused, Service,
    abi::workflow::{ControlInput, ControlOutput, ControlValue},
    control, workflow,
};
use berm_api::{
    Meter, Output, digest,
    host::{Kind as UsageKind, Link, Outcome as UsageOutcome, Source as UsageSource, User},
    schema,
    workflow::{
        Offer, Version,
        graph::{Entry, Kind, Mode, Node, Producer, Source, Target},
        run::{
            Accepted, Binding, Checkpoint, Failure, Item, Prepared, PreparedCall, Run, State,
            Status, Trigger,
        },
        spend::Context as SpendContext,
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

struct Execution<'a> {
    claim: &'a Claim,
    deadline: Instant,
    total: Meter,
}

impl<E: Env> Host<E> {
    pub fn workflow_offer(&self, service: &str, workflow: &str) -> Result<Offer> {
        let owner = self.deployed(service)?;
        let spec = owner
            .manifest
            .workflows
            .iter()
            .find(|spec| spec.entry.name == workflow)
            .ok_or_else(|| deny(StatusCode::NOT_FOUND, "no such workflow"))?;
        Ok(Offer {
            source: Source {
                image: digest::prefixed(&owner.digest),
                graph: spec.entry.digest()?,
            },
            local_bindings: spec
                .entry
                .graph
                .nodes
                .iter()
                .filter_map(|node| match &node.kind {
                    Kind::Step { call, .. } if matches!(call.target, Target::Local { .. }) => Some(
                        owner
                            .binding(call.target.clone())
                            .map(|binding| (node.id.clone(), binding)),
                    ),
                    _ => None,
                })
                .collect::<Result<_>>()?,
            spec: spec.clone(),
            control_budget: owner.controls[workflow],
            control_buffer: owner.manifest.buffer as u64,
            deadline_ms: self.deadline.as_millis().try_into()?,
        })
    }

    pub fn workflow_binding(&self, service: &str, tool: &str) -> Result<Binding> {
        self.workflow_available(service)?;
        let deployed = self.deployed(service)?;
        if !deployed.public(tool) {
            return Err(deny(StatusCode::NOT_FOUND, "no such public tool").into());
        }
        if !deployed.reached_by_services(tool) {
            return Err(deny(
                StatusCode::FORBIDDEN,
                "workflow target takes no calls from services",
            )
            .explained(Denied::SERVICE_CALLERS)
            .into());
        }
        deployed.binding(Target::Service {
            service: service.into(),
            tool: tool.into(),
            idempotency_ms: None,
        })
    }

    /// Resolve locally deployed dependencies without executing control functions or tools.
    pub fn workflow_bindings(&self, service: &str, workflow: &str) -> Result<Accepted> {
        let owner = self.deployed(service)?;
        let spec = owner
            .manifest
            .workflows
            .iter()
            .find(|spec| spec.entry.name == workflow)
            .ok_or_else(|| deny(StatusCode::NOT_FOUND, "no such workflow"))?;
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
        let accepted = self.workflow_bindings(service, workflow)?;
        self.prepare_accepted_workflow(service, workflow, input, trigger, accepted)
    }

    pub fn prepare_accepted_workflow(
        &self,
        service: &str,
        workflow: &str,
        input: Value,
        trigger: Trigger,
        accepted: Accepted,
    ) -> Result<Run> {
        self.workflow_available(service)?;
        let owner = self.deployed(service)?;
        let spec = owner
            .manifest
            .workflows
            .iter()
            .find(|spec| spec.entry.name == workflow)
            .ok_or_else(|| deny(StatusCode::NOT_FOUND, "no such workflow"))?;
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
            accepted,
            status: Status::Pending,
            attempts: vec![],
            checkpoints: BTreeMap::new(),
            result: None,
            error: None,
        })
    }

    pub async fn preflight_workflow(&self, id: &str) -> Result<()> {
        let run = self
            .workflows
            .get(id)
            .await?
            .ok_or(LedgerRefused::Missing)?;
        self.workflow_owner(&run).await?;
        match self
            .workflows
            .view(id, (self.now)())
            .await?
            .ok_or(LedgerRefused::Missing)?
            .continuation
        {
            berm_api::workflow::view::Continuation::Available => Ok(()),
            berm_api::workflow::view::Continuation::Unavailable { reason } => Err(reason.into()),
        }
    }

    /// The embedder authorizes admission; this entry starts a pending, accepted run.
    pub async fn claim_workflow(&self, id: &str, intent: Intent) -> Result<Claim> {
        let run = self
            .workflows
            .get(id)
            .await?
            .ok_or(LedgerRefused::Missing)?;
        let now = (self.now)();
        if intent == Intent::Continue {
            match self.workflows.checkpoint_eligibility(id, now).await? {
                Eligibility::Available => {}
                Eligibility::Busy => return Err(LedgerRefused::Busy.into()),
                Eligibility::Expired => return Err(LedgerRefused::Expired.into()),
                Eligibility::Removed => return Err(LedgerRefused::Removed.into()),
                Eligibility::Stopped => {
                    return Err(deny(
                        StatusCode::SERVICE_UNAVAILABLE,
                        "workflow service is stopped",
                    )
                    .explained(Denied::PAUSED)
                    .into());
                }
                Eligibility::NotFailed => return Err(LedgerRefused::Ineligible.into()),
            }
            self.workflow_owner(&run).await?;
            for checkpoint in run.checkpoints.values() {
                for item in &checkpoint.items {
                    if matches!(item.state, State::Unknown { .. }) {
                        for dispatch in item.dispatches.iter().rev() {
                            if let Some(receipt) = self
                                .peers
                                .receipt(&run.service, &run.id, &dispatch.id)
                                .await?
                            {
                                self.workflows.reconcile(id, receipt, (self.now)()).await?;
                                break;
                            }
                        }
                    }
                }
            }
        }
        let now = (self.now)();
        let duration = Duration::from_millis(run.accepted.deadline_ms).min(self.deadline);
        self.workflows
            .claim(
                id,
                intent,
                now,
                now.checked_add(duration.as_millis().try_into()?)
                    .context("workflow deadline overflow")?,
            )
            .await
    }

    pub async fn execute_workflow(&self, id: &str) -> Result<Run> {
        let claim = self.claim_workflow(id, Intent::Start).await?;
        self.execute_claimed_workflow(claim).await
    }

    pub async fn continue_workflow(&self, id: &str) -> Result<Run> {
        let claim = self.claim_workflow(id, Intent::Continue).await?;
        self.execute_claimed_workflow(claim).await
    }

    pub async fn cancel_workflow(&self, id: &str) -> Result<Run> {
        self.workflows.cancel(id, (self.now)()).await
    }

    async fn workflow_owner(&self, run: &Run) -> Result<Arc<Service>> {
        self.workflow_available(&run.service)?;
        let bytes = self.workflows.image(&run.id).await?;
        let owner = self.berm.retained(&run.service, &bytes)?;
        for binding in run.accepted.bindings.values() {
            let mut current = match binding.target {
                Target::Local { .. } => owner.binding(binding.target.clone())?,
                Target::Service { .. }
                    if host::split(&binding.service).0 == host::split(&run.service).0 =>
                {
                    self.workflow_binding(&binding.service, binding.target.tool())?
                }
                Target::Service { .. } => {
                    self.peers
                        .binding(&run.service, &binding.service, binding.target.tool())
                        .await?
                }
            };
            current.target = binding.target.clone();
            if current != *binding {
                return Err(
                    deny(StatusCode::CONFLICT, "workflow dependency binding changed")
                        .explained(Binding::CHANGED)
                        .into(),
                );
            }
        }
        Ok(owner)
    }

    /// The embedder has claimed the attempt and authorized its spend context.
    pub async fn execute_claimed_workflow(&self, claim: Claim) -> Result<Run> {
        if !self.workflows.launch(&claim, (self.now)()).await? {
            return Err(LedgerRefused::Busy.into());
        }
        self.execute_started_workflow(claim).await
    }

    pub async fn launch_workflow(&self, claim: Claim, context: Option<SpendContext>) -> Result<()> {
        if self.workflows.launch(&claim, (self.now)()).await? {
            let host = self.clone();
            tokio::spawn(async move {
                let result = match context {
                    Some(context) => {
                        workflow::scope(context, host.execute_started_workflow(claim)).await
                    }
                    None => host.execute_started_workflow(claim).await,
                };
                if let Err(error) = result {
                    tracing::error!(%error,"workflow execution interrupted");
                }
            });
        }
        Ok(())
    }

    async fn execute_started_workflow(&self, claim: Claim) -> Result<Run> {
        let run = self
            .workflows
            .get(&claim.run)
            .await?
            .context("no such workflow run")?;
        let now = (self.now)();
        if claim.deadline_at <= now {
            return Err(
                deny(StatusCode::CONFLICT, "workflow attempt deadline expired")
                    .explained(SpendContext::EXPIRED)
                    .into(),
            );
        }
        let duration = Duration::from_millis(claim.deadline_at - now)
            .min(Duration::from_millis(run.accepted.deadline_ms))
            .min(self.deadline);
        self.execute_attempt(run, claim, Instant::now() + duration)
            .await
    }

    pub(crate) async fn execute_attempt(
        &self,
        mut run: Run,
        claim: Claim,
        deadline: Instant,
    ) -> Result<Run> {
        if let Some(context) = workflow::current()
            && (context.run != run.id
                || context.attempt != claim.attempt
                || context.deadline_at != claim.deadline_at
                || context.owner != host::split(&run.service).0)
        {
            return Err(deny(
                StatusCode::CONFLICT,
                "workflow spend authorization belongs to another attempt",
            )
            .explained(SpendContext::EXPIRED)
            .into());
        }
        let mut execution = Execution {
            claim: &claim,
            deadline,
            total: Meter::default(),
        };
        // Take the claimed state, including its durable attempt identity.
        run = self
            .workflows
            .get(&run.id)
            .await?
            .context("workflow disappeared")?;
        let executed = async {
            let _slot = tokio::time::timeout_at(deadline.into(), self.gate.acquire()).await??;
            let owner = self.workflow_owner(&run).await?;
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
                self.workflows.active(&claim, (self.now)()).await?;
                self.workflow_available(&run.service)?;
                if Self::skip_node(&run, node)? {
                    let mut checkpoint = run.checkpoints[&node.id].clone();
                    checkpoint.state = State::Skipped;
                    self.save_node(&claim, &mut run, node, checkpoint).await?;
                    continue;
                }
                if let Err(error) = self
                    .execute_node(&owner, &entry, &mut run, node, &mut execution)
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
                        execution.total,
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
                    .finish(
                        &claim,
                        status,
                        None,
                        Some(failure),
                        execution.total,
                        (self.now)(),
                    )
                    .await?;
            }
        }
        self.workflows
            .get(&run.id)
            .await?
            .context("workflow disappeared")
    }

    pub(crate) fn workflow_available(&self, service: &str) -> Result<()> {
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
            let value = match &run.checkpoints[&guard.producer].state {
                State::Succeeded { value } => Some(value),
                _ => None,
            };
            if !guard.selection.selects(value) {
                return Ok(true);
            }
        }
        Ok(false)
    }

    async fn execute_node(
        &self,
        owner: &Arc<Service>,
        entry: &Entry,
        run: &mut Run,
        node: &Node,
        execution: &mut Execution<'_>,
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
                    deadline: execution.deadline,
                    meters: meters.clone(),
                })
                .await;
            let used = self.metered(meters.take());
            checkpoint.meter += used;
            execution.total += used;
            let result = match result {
                Ok(value) => value,
                Err(error) => {
                    let failure = Self::execution_failure(&error);
                    checkpoint.state = State::Failed {
                        error: failure.clone(),
                    };
                    self.save_node(execution.claim, run, node, checkpoint)
                        .await?;
                    return Err(Failed(failure).into());
                }
            };
            let prepared = (|| -> Result<()> {
                match (&node.kind, result) {
                    (Kind::Step { call, mode, .. }, ControlOutput::Calls { inputs }) => {
                        ensure!(
                            inputs.len() <= mode.maximum() as usize
                                && (matches!(mode, Mode::Each { .. }) || inputs.len() == 1),
                            "workflow builder exceeded its declared call bound"
                        );
                        let binding = &run.accepted.bindings[&node.id];
                        let input_schema = schema::Validator::try_from(&call.input)?;
                        let binding_schema = schema::Validator::try_from(&binding.input)?;
                        let mut calls = Vec::with_capacity(inputs.len());
                        for input in inputs {
                            input_schema.validate(&input)?;
                            binding_schema.validate(&input)?;
                            let operation_key = input
                                .get(PreparedCall::IDEMPOTENCY_KEY)
                                .and_then(Value::as_str)
                                .filter(|key| !key.is_empty())
                                .map(str::to_owned);
                            if binding.idempotency_ms.is_some() {
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
                                receipts: vec![],
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
                self.save_node(execution.claim, run, node, checkpoint)
                    .await?;
                return Err(Failed(failure).into());
            }
            if matches!(node.kind, Kind::Output) {
                run.checkpoints.insert(node.id.clone(), checkpoint);
                return Ok(());
            }
            self.save_node(execution.claim, run, node, checkpoint.clone())
                .await?;
        }
        let Some(Prepared::Calls { calls }) = checkpoint.prepared.clone() else {
            return Ok(());
        };
        let Kind::Step { mode, call, .. } = &node.kind else {
            bail!("call checkpoint belongs to a non-step")
        };
        let output_schema = schema::Validator::try_from(&call.output)?;
        for (index, prepared) in calls.iter().enumerate() {
            if matches!(checkpoint.items[index].state, State::Succeeded { .. }) {
                continue;
            }
            self.workflows.active(execution.claim, (self.now)()).await?;
            self.workflow_available(&run.service)?;
            ensure!(
                Instant::now() < execution.deadline,
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
                self.workflow_available(&binding.service)?;
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
                .begin_dispatch(execution.claim, &node.id, index, (self.now)())
                .await
                .map_err(|error| {
                    if error.is::<LedgerRefused>() {
                        error
                    } else {
                        Persistence(error).into()
                    }
                })?;
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
            let replay = checkpoint.items[index]
                .dispatches
                .last()
                .unwrap()
                .operation
                .clone();
            let mut launched = None;
            let result = if let Some(target) = target {
                let link = InvocationLink {
                    caller: Some(Caller::Service(run.service.as_str().into())),
                    key: identity
                        .and_then(|caller| caller.key.as_deref())
                        .map(Into::into),
                    user: user.as_ref().map(host::user),
                    client: Client::default(),
                    depth: 1,
                    query: false,
                    deadline: execution.deadline,
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
                    retained_until: Some(run.expires_at),
                    replay: replay.clone(),
                    workflow: workflow::current(),
                    operation: checkpoint.items[index]
                        .dispatches
                        .last()
                        .map(|dispatch| dispatch.id.clone()),
                    accepted: Some(binding.clone()),
                    caller: Some(run.service.clone()),
                    from: run.service.clone(),
                    key: identity.and_then(|caller| caller.key.clone()),
                    user,
                    client: None,
                    client_ip: None,
                    depth: 1,
                    query: false,
                    timeout: execution
                        .deadline
                        .saturating_duration_since(Instant::now())
                        .as_millis()
                        .try_into()?,
                    stack: vec![run.service.clone(), binding.service.clone()],
                };
                self.peers
                    .call(&binding.service, call.target.tool(), args, link)
                    .await
                    .map(|reply| {
                        launched = reply.launched;
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
            execution.total += used;
            checkpoint.meter += used;
            if local {
                launched = Some(used.calls > 0);
            }
            if !local
                && let Err(error) = &result
                && let Some(failure) = error.downcast_ref::<Failure>()
            {
                launched = failure.launched();
            }
            let mut state = match result {
                Ok((Output::Done(text), _)) => match serde_json::from_str(&text)
                    .map_err(anyhow::Error::from)
                    .and_then(|value| {
                        output_schema.validate(&value)?;
                        Ok(value)
                    }) {
                    Ok(value) => State::Succeeded { value },
                    Err(error) => State::Unknown {
                        error: Self::execution_failure(&error),
                    },
                },
                Ok((Output::Failed(message), denied)) => {
                    let mut error = message.parse::<Failure>().unwrap_or_else(|_| Failure {
                        status: 422,
                        headers: BTreeMap::new(),
                        body: json!({"error":message}),
                    });
                    if denied && error.status == 422 {
                        error.status = 403;
                    }
                    if error.status >= 500 {
                        State::Unknown { error }
                    } else {
                        State::Failed { error }
                    }
                }
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
            if launched == Some(false)
                && let State::Unknown { error } = &state
            {
                state = State::Failed {
                    error: error.clone(),
                };
            }
            let dispatch = checkpoint.items[index].dispatches.last_mut().unwrap();
            dispatch.launched = launched;
            dispatch.state = state.clone();
            dispatch.finished_at = Some((self.now)());
            dispatch.meter = used;
            if replay.is_some()
                && let State::Failed { error } = &state
            {
                state = State::Unknown {
                    error: error.clone(),
                };
            }
            checkpoint.items[index].state = state.clone();
            if let State::Failed { error } | State::Unknown { error } = &state {
                checkpoint.state = state.clone();
                let error = error.clone();
                self.save_node(execution.claim, run, node, checkpoint)
                    .await?;
                return Err(Failed(error).into());
            }
            self.save_node(execution.claim, run, node, checkpoint.clone())
                .await?;
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
        self.save_node(execution.claim, run, node, checkpoint).await
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
        if let Some(refused) = error.downcast_ref::<LedgerRefused>() {
            return Failure::from(&Denied::from(refused.clone()));
        }

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
