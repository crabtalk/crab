use crate::{caller::Identity, format, runtime, terminal};
use anyhow::{Context, Result, ensure};
use berm_api::{
    address::Address,
    schema,
    workflow::{
        graph::{Kind, Producer, Target},
        run::Status,
        view::{Continuation, Start},
    },
};
use clap::{Args, Subcommand};
use crabtalk_sdk::{
    client::Client,
    format as display,
    services::workflow::{Inspection, Run},
};
use std::{
    io::{self, Write},
    process::ExitCode,
    str::FromStr,
    time::Duration,
};
use uuid::Uuid;

#[derive(Args)]
pub struct Workflow {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Resolve the graph, bounds and current quote without executing it.
    #[command(alias = "dry-run")]
    Inspect {
        /// A service and workflow, `owner/service.workflow` on Cloud.
        target: Reference,
        #[command(flatten)]
        identity: Identity,
    },
    /// Admit one run and return its identity; use status to follow it.
    Run {
        target: Reference,
        /// JSON input; read stdin when omitted, or use {} with an empty stdin.
        input: Option<String>,
        /// Inspect and validate input without creating a run or charging anything.
        #[arg(long)]
        dry_run: bool,
        /// A UUID to reuse after a lost response, with identical input and terms.
        #[arg(long)]
        id: Option<Uuid>,
        #[command(flatten)]
        identity: Identity,
    },
    /// Read a run's checkpoints, result and continuation eligibility.
    Status { target: Reference, run: Uuid },
    /// Continue an eligible run using its original input and saved checkpoints.
    Continue { target: Reference, run: Uuid },
    /// Stop new calls; an external request already accepted cannot be recalled.
    Cancel { target: Reference, run: Uuid },
}

#[derive(Clone)]
pub struct Reference {
    pub service: String,
    pub workflow: String,
}

impl FromStr for Reference {
    type Err = anyhow::Error;

    fn from_str(text: &str) -> Result<Self> {
        let (service, workflow) = text
            .rsplit_once('.')
            .context("use service.workflow, or owner/service.workflow on Cloud")?;
        let address = if service.contains('/') {
            service.to_owned()
        } else {
            format!("local/{service}")
        };
        address.parse::<Address>()?;
        ensure!(
            !workflow.is_empty()
                && workflow
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_')),
            "a workflow name uses ASCII letters, digits, `-` and `_`"
        );
        Ok(Self {
            service: service.into(),
            workflow: workflow.into(),
        })
    }
}

impl Workflow {
    pub fn run(&self, client: &Client, json: bool, yes: bool) -> Result<ExitCode> {
        let run = match &self.command {
            Command::Inspect { target, identity } => {
                let caller = identity.resolve(client)?;
                let inspection = runtime::wait(client.workflow(
                    &target.service,
                    &target.workflow,
                    caller.as_ref(),
                ))?;
                if json {
                    terminal::json(&inspection)?;
                } else {
                    show_inspection(&inspection, io::stdout().lock())?;
                }
                return Ok(ExitCode::SUCCESS);
            }
            Command::Run {
                target,
                input,
                dry_run,
                id,
                identity,
            } => {
                let caller = identity.resolve(client)?;
                let bytes = terminal::arguments(input.as_deref())?;
                let input = serde_json::from_slice(if bytes.is_empty() { b"{}" } else { &bytes })
                    .context("workflow input must be JSON")?;
                let inspection = runtime::wait(client.workflow(
                    &target.service,
                    &target.workflow,
                    caller.as_ref(),
                ))?;
                schema::validate(&inspection.definition.entry.graph.input, &input)?;
                if *dry_run {
                    if json {
                        terminal::json(&inspection)?;
                    } else {
                        show_inspection(&inspection, io::stdout().lock())?;
                    }
                    return Ok(ExitCode::SUCCESS);
                }
                ensure!(
                    client.owner.is_some() || caller.is_some(),
                    "workflow runs require a stable local caller; omit --anonymous"
                );
                show_inspection(&inspection, io::stderr().lock())?;
                terminal::confirm(yes, "Start this workflow run")?;
                let id = id.unwrap_or_else(Uuid::new_v4).to_string();
                eprintln!("run {id}");
                eprintln!(
                    "If the response is lost, use crab workflow status {}.{} {id}; retry run only with --id {id} and the same input and terms.",
                    target.service, target.workflow
                );
                runtime::wait(client.start_workflow(
                    &target.service,
                    &target.workflow,
                    &Start {
                        id,
                        quote: inspection.definition.quote,
                        input,
                    },
                    caller.as_ref(),
                ))?
            }
            Command::Status { target, run } => runtime::wait(client.workflow_run(
                &target.service,
                &target.workflow,
                &run.to_string(),
            ))?,
            Command::Continue { target, run } => {
                let previous = runtime::wait(client.workflow_run(
                    &target.service,
                    &target.workflow,
                    &run.to_string(),
                ))?;
                show_run(&previous, io::stderr().lock())?;
                if let Continuation::Unavailable { reason } = &previous.execution.continuation {
                    return Err(reason.clone()).context("continuation unavailable");
                }
                show_inspection(&previous.accepted, io::stderr().lock())?;
                terminal::confirm(
                    yes,
                    "Continue this workflow with its remaining owner spend allowance",
                )?;
                runtime::wait(client.continue_workflow(
                    &target.service,
                    &target.workflow,
                    &run.to_string(),
                ))?
            }
            Command::Cancel { target, run } => {
                terminal::confirm(
                    yes,
                    "Cancel this workflow run; already accepted external requests may still finish",
                )?;
                runtime::wait(client.cancel_workflow(
                    &target.service,
                    &target.workflow,
                    &run.to_string(),
                ))?
            }
        };
        if json {
            terminal::json(&run)?;
        } else {
            show_run(&run, io::stdout().lock())?;
        }
        Ok(match (&self.command, run.execution.status) {
            (Command::Cancel { .. }, _) => ExitCode::SUCCESS,
            (_, Status::Failed | Status::Indeterminate | Status::Cancelled) => ExitCode::from(2),
            _ => ExitCode::SUCCESS,
        })
    }
}

pub fn show_inspection(inspection: &Inspection, mut output: impl Write) -> Result<()> {
    let definition = &inspection.definition;
    writeln!(output, "{}.{}", definition.service, definition.entry.name)?;
    if let Some(title) = &definition.title {
        writeln!(output, "  {title}")?;
    }
    match definition.task_price {
        Some(price) => writeln!(
            output,
            "  fixed task price: {} (successful result only)",
            format::usd(price as f64)
        )?,
        None => writeln!(output, "  fixed task price: none")?,
    }
    writeln!(
        output,
        "  attempt deadline: {}; checkpoint retention: {}",
        humantime::format_duration(Duration::from_millis(definition.deadline_ms)),
        humantime::format_duration(Duration::from_millis(definition.retention_ms))
    )?;
    if let Some(terms) = &inspection.accounting {
        writeln!(
            output,
            "  owner spend cap: {} across all attempts",
            format::usd(terms.max_cost as f64)
        )?;
        writeln!(
            output,
            "  direct-call/control quote: {}; nested calls also consume the cap",
            format::usd(terms.direct_cost as f64)
        )?;
        writeln!(
            output,
            "  control budget: {}µs CPU, {} rows read, {} written, {} memory",
            terms.execution.control_budget.cpu,
            terms.execution.control_budget.read,
            terms.execution.control_budget.written,
            display::bytes(terms.execution.control_budget.memory)
        )?;
        writeln!(
            output,
            "  checkpoint reservation: {}; rate revision: {}",
            display::bytes(terms.execution.checkpoint_bytes),
            terms.rate_revision
        )?;
        for account in &terms.external_billing {
            writeln!(output, "  provider billing outside managed cap: {account}")?;
        }
    }
    if inspection.accounting.is_none() {
        writeln!(
            output,
            "  owner spend cap and accounting: not included in this inspection"
        )?;
    }
    writeln!(output, "  input: {}", definition.entry.graph.input)?;
    for node in &definition.entry.graph.nodes {
        let dependencies = node
            .dependencies
            .iter()
            .map(|dependency| {
                let mut name = match &dependency.source {
                    Producer::Input => "input".into(),
                    Producer::Node { id } => id.clone(),
                };
                if let Some(case) = &dependency.case {
                    name.push_str(&format!(".{case}"));
                }
                if dependency.optional {
                    name.push('?');
                }
                name
            })
            .collect::<Vec<_>>()
            .join(", ");
        match &node.kind {
            Kind::Step { call, mode, price } => {
                let target = match &call.target {
                    Target::Local { tool } => format!("self.{tool}"),
                    Target::Service { service, tool, .. } => format!("{service}.{tool}"),
                };
                writeln!(
                    output,
                    "  {} <- [{dependencies}]: {target}; {mode:?}, at most {} calls",
                    node.id,
                    mode.maximum()
                )?;
                if let Some(price) = price {
                    writeln!(
                        output,
                        "    price: {}",
                        format::step_price(&definition.entry.graph, node, *price)
                    )?;
                }
                if let Some(terms) = &inspection.accounting {
                    if let Some(binding) = terms.execution.bindings.get(&node.id) {
                        writeln!(
                            output,
                            "    image {}; idempotency window: {}",
                            binding.image,
                            binding.idempotency_ms.map_or_else(
                                || "none".into(),
                                |ms| humantime::format_duration(Duration::from_millis(ms))
                                    .to_string()
                            )
                        )?;
                    }
                    if let Some(dependency) = terms.dependencies.get(&node.id) {
                        writeln!(
                            output,
                            "    tool price: {}; reserved allowance: {}",
                            dependency
                                .price
                                .map_or_else(|| "none".into(), |price| price.to_string()),
                            format::usd(dependency.allowance as f64)
                        )?;
                    }
                }
            }
            Kind::Branch { cases } => writeln!(
                output,
                "  {} <- [{dependencies}]: branch [{}]",
                node.id,
                cases
                    .iter()
                    .map(|case| case.name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            )?,
            Kind::Output => writeln!(output, "  {} <- [{dependencies}]: output", node.id)?,
        }
        for guard in &node.guards {
            writeln!(
                output,
                "    guard {}: {:?}",
                guard.producer, guard.selection
            )?;
        }
    }
    writeln!(output, "  image {}", definition.source.image)?;
    writeln!(output, "  graph {}", definition.source.graph)?;
    writeln!(
        output,
        "  quote {}; admission revalidates current terms",
        definition.quote
    )?;
    Ok(())
}

pub fn show_run(run: &Run, mut output: impl Write) -> Result<()> {
    let execution = &run.execution;
    writeln!(
        output,
        "{}.{} run {}: {:?}",
        execution.service, execution.workflow, execution.id, execution.status
    )?;
    writeln!(
        output,
        "  checkpoints expire {}",
        format::instant(execution.expires_at)
    )?;
    match run.accepted.definition.task_price {
        Some(price) => writeln!(
            output,
            "  fixed task price: {} (successful result only)",
            format::usd(price as f64)
        )?,
        None => writeln!(output, "  fixed task price: none")?,
    }
    writeln!(output, "  attempts: {}", execution.attempts.len())?;
    match &execution.continuation {
        Continuation::Available => writeln!(output, "  continuation available")?,
        Continuation::Unavailable { reason } => writeln!(
            output,
            "  continuation unavailable ({}): {}",
            reason.status, reason.body
        )?,
    }
    if let Some(accounting) = &run.accounting {
        writeln!(
            output,
            "  owner spend: {} incurred, {} reserved, {} cap",
            format::usd(accounting.spend.incurred.parse()?),
            format::usd(accounting.spend.reserved.parse()?),
            format::usd(accounting.terms.max_cost as f64)
        )?;
        writeln!(
            output,
            "  task settlement: {}",
            serde_json::to_string(&accounting.settlement)?
        )?;
    }
    for (id, checkpoint) in &execution.checkpoints {
        writeln!(output, "  {id}: {}", checkpoint.state)?;
        for item in &checkpoint.items {
            writeln!(
                output,
                "    item {}: {} ({} dispatches)",
                item.index,
                item.state,
                item.dispatches.len()
            )?;
        }
    }
    if let Some(result) = &execution.result {
        writeln!(output, "  result: {result}")?;
    }
    if let Some(error) = &execution.error {
        writeln!(output, "  error ({}): {}", error.status, error.body)?;
    }
    Ok(())
}
