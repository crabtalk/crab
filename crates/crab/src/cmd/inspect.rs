//! `crab inspect` — what a service claims to be.

use crate::{format, project, runtime, terminal};
use anyhow::{Context, Result};
use berm_api::{Access, Manifest, Runtime, Service, ToolSpec, digest};
use crabtalk_sdk::{
    client::{Client, Error},
    format as display,
    services::service::{Estimate, Measured, Terms},
    system::meta::Metering,
};
use serde_json::Value;
use std::{fs, path::Path};

/// Print `name` as its caller sees it: the runtime too when the host answers
/// it, which Cloud does for the service's owners alone, and what one run of
/// each tool costs at most when `cloud` answers with its rates.
pub fn run(client: &Client, cloud: &Client, name: &str, json: bool) -> Result<()> {
    let service = runtime::wait(client.inspect(name))?;
    let runtime = match runtime::wait(client.runtime(name)) {
        Ok(runtime) => Some(runtime),
        Err(Error::Api { status: 403, .. }) => None,
        Err(e) => return Err(e.into()),
    };
    let terms = runtime::wait(client.terms(name))?;
    if json {
        return terminal::json(&serde_json::json!({
            "service": service,
            "runtime": runtime,
            "terms": terms,
        }));
    }
    let metering = runtime::wait(cloud.rates()).ok();
    show(
        &service,
        runtime.as_ref(),
        &terms,
        metering.as_ref(),
        runtime
            .as_ref()
            .map(|runtime| runtime.workflows.as_slice())
            .unwrap_or_default(),
    );
    Ok(())
}

pub fn directory(dir: &Path, packages: &[String], json: bool) -> Result<()> {
    anyhow::ensure!(dir.is_dir(), "inspect expects a service directory");
    let dir = dir
        .canonicalize()
        .context("cannot resolve the service directory")?;
    let selection = project::Selection::read(&dir, packages, true, true)?;
    anyhow::ensure!(
        !selection.services.is_empty(),
        "no service in this directory"
    );
    let mut manifests = Vec::new();
    for source in selection.services {
        let image = berm_build::build(&source.dir, None).context("service build failed")?;
        let bytes = fs::read(image).context("cannot read the built service image")?;
        let manifest = Manifest::try_from(bytes.as_slice()).context("invalid service image")?;
        if !json {
            let service = Service {
                name: manifest.name.clone(),
                version: manifest.version.clone(),
                description: manifest.description.clone(),
                homepage: manifest.homepage.clone(),
                repository: manifest.repository.clone(),
                readme: manifest.readme.clone(),
                logo: manifest.logo.clone(),
                digest: digest::prefixed(&digest::hex(&bytes)),
                tools: manifest.tools.clone(),
                workflows: manifest
                    .workflows
                    .iter()
                    .filter_map(|workflow| workflow.public())
                    .collect(),
                vars: manifest.deps.vars.clone(),
            };
            show(&service, None, &Terms::default(), None, &manifest.workflows);
        }
        manifests.push(manifest);
    }
    if json {
        if manifests.len() == 1 {
            terminal::json(&manifests[0])?;
        } else {
            terminal::json(&manifests)?;
        }
    }
    Ok(())
}

pub fn show(
    service: &Service,
    runtime: Option<&Runtime>,
    terms: &Terms,
    metering: Option<&Metering>,
    workflows: &[berm_api::workflow::config::Spec],
) {
    println!("{}", service.name);
    if !service.description.is_empty() {
        println!("  {}", service.description);
    }
    if !service.version.is_empty() {
        println!("  version {}", service.version);
    }
    println!("  digest  {}", service.digest);
    for (label, value) in [
        ("homepage", &service.homepage),
        ("repository", &service.repository),
    ] {
        if !value.is_empty() {
            println!("  {label} {value}");
        }
    }
    if !service.readme.is_empty() {
        println!();
        for line in service.readme.lines() {
            println!("  {line}");
        }
    }
    if workflows.is_empty() {
        for workflow in &service.workflows {
            println!(
                "  workflow {}: {}",
                workflow.entry.name,
                workflow.title.as_deref().unwrap_or_default()
            );
            println!("    access {}", workflow.access);
            println!("    price {} USD micros", workflow.price.unwrap_or(0));
        }
    } else {
        for workflow in workflows {
            println!(
                "  workflow {}: {}",
                workflow.entry.name,
                workflow.config.title.as_deref().unwrap_or_default()
            );
            println!(
                "    access {}",
                workflow.config.access.unwrap_or(Access::Internal)
            );
            println!(
                "    price {} USD micros",
                workflow.config.price.unwrap_or(0)
            );
            println!("    max_cost {} USD micros", workflow.config.max_cost);
            println!("    checkpoint_bytes {}", workflow.config.checkpoint_bytes);
            println!("    retain {} seconds", workflow.config.retain.as_secs());
            let budget = workflow
                .config
                .control_budget(berm_host::Config::default().budget);
            println!(
                "    control budget: {}µs CPU, {} memory",
                budget.cpu,
                display::bytes(budget.memory)
            );
        }
    }
    let Some(runtime) = runtime else {
        tools(service.tools.iter(), terms, metering);
        return;
    };
    let answered = |missing: bool| match missing {
        true => "  (nothing answers to it here)",
        false => "",
    };
    println!("  berm-lang {}", runtime.lang);
    println!("  host calls {}", runtime.imports.join(", "));
    for (name, hosts) in [
        ("fetch", &runtime.deps.http.fetch),
        ("proxy", &runtime.deps.http.proxy),
    ] {
        if !hosts.is_empty() {
            println!("  http {name} -> {}", hosts.join(", "));
        }
    }
    if runtime.deps.secret_resources {
        println!("  runtime secrets -> explicit service grants");
    }
    for name in &runtime.deps.secrets {
        let missing = runtime.unresolved.secrets.contains(name);
        let note = match missing && runtime.deps.optional(name) {
            true => "  (optional, not set here)",
            false => answered(missing),
        };
        println!("  secret  {name}{note}");
    }
    for (name, var) in &runtime.deps.vars {
        println!("  var     {name} = {:?}", var.default);
    }

    tools(service.tools.iter().chain(&runtime.tools), terms, metering);
    for event in &runtime.events {
        println!();
        match event.deprecated {
            true => println!("  event {} (deprecated)", event.name),
            false => println!("  event {}", event.name),
        }
        println!("    {}", event.description);
        for field in properties(&event.fields) {
            println!("    {field}");
        }
    }
}

/// `tools` as a table of each one's access, title, catalog job, price, marks
/// and description, then each one's arguments, reply, and what one call of it
/// is held to and costs.
fn tools<'a>(
    tools: impl Iterator<Item = &'a ToolSpec>,
    terms: &Terms,
    metering: Option<&Metering>,
) {
    let tools: Vec<&ToolSpec> = tools.collect();
    let rows: Vec<[String; 7]> = tools
        .iter()
        .map(|tool| {
            let access = match tool.access() {
                None => "internal".to_owned(),
                Some(access @ (Access::Accounts | Access::Services)) => {
                    format!("restricted: {access}")
                }
                Some(access) => access.to_string(),
            };
            let marks: Vec<&str> = [
                (tool.query, "query"),
                (tool.cron, "cron"),
                (tool.stripe, "stripe"),
                (tool.webhook, "webhook"),
                (tool.deprecated, "deprecated"),
            ]
            .into_iter()
            .filter_map(|(set, mark)| set.then_some(mark))
            .collect();
            let price = terms
                .tools
                .get(&tool.name)
                .and_then(|terms| terms.price.map(u64::from))
                .or(tool.price)
                .unwrap_or(0);
            let price = Some(format!("{price} USD micros"));
            let dash = |value: Option<String>| value.unwrap_or_else(|| "-".to_owned());
            [
                tool.name.clone(),
                access,
                dash(tool.title.clone()),
                dash(tool.catalog.clone()),
                dash(price),
                dash((!marks.is_empty()).then(|| marks.join(", "))),
                tool.description
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" "),
            ]
        })
        .collect();
    let head = [
        "TOOL",
        "ACCESS",
        "TITLE",
        "CATALOG",
        "PRICE",
        "MARKS",
        "DESCRIPTION",
    ]
    .map(str::to_owned);
    let mut widths = [0; 6];
    for row in std::iter::once(&head).chain(&rows) {
        for (width, cell) in widths.iter_mut().zip(row) {
            *width = (*width).max(cell.chars().count());
        }
    }
    println!();
    for row in std::iter::once(&head).chain(&rows) {
        let mut line = String::from(" ");
        for (width, cell) in widths.iter().zip(row) {
            line.push_str(&format!(" {cell:<width$} "));
        }
        println!("{line} {}", row[6]);
    }

    for tool in tools {
        println!();
        println!("  {}", tool.name);
        if let Some(held) = terms.tools.get(&tool.name) {
            let budget = held.held.budget;
            println!(
                "    at most {}µs of CPU, {} rows read, {} written and {} of memory a call",
                budget.cpu,
                budget.read,
                budget.written,
                display::bytes(budget.memory)
            );
            if let Some(metering) = metering {
                let most = metering.most(&budget, terms.deadline);
                println!(
                    "    at most {} to run, its calls to other services aside",
                    format::usd(most.parse().unwrap_or(0.0))
                );
            }
            if let Some(estimate) = &held.estimate {
                let cost = |m: &Measured| format::usd(m.cost_micros_decimal.parse().unwrap_or(0.0));
                let slow = if estimate.runs >= Estimate::P99 {
                    "at p99"
                } else {
                    "at the slowest"
                };
                println!(
                    "    {} to run typically, {} {slow}, over {} runs",
                    cost(&estimate.typical),
                    cost(&estimate.slow),
                    format::count(estimate.runs)
                );
            }
        }
        if let Some(window) = tool.idempotency_ms {
            println!("    idempotency {window}ms");
        }
        for example in &tool.examples {
            println!("    example {example}");
        }
        if !terms.tools.contains_key(&tool.name) {
            let budget = tool
                .budget
                .unwrap_or_default()
                .over(berm_host::Config::default().budget);
            println!(
                "    at most {}µs of CPU, {} rows read, {} written and {} of memory a call",
                budget.cpu,
                budget.read,
                budget.written,
                display::bytes(budget.memory)
            );
        }
        for parameter in properties(&tool.parameters) {
            println!("    {parameter}");
        }
        if let Some(returns) = &tool.returns {
            println!("    returns {}", format::schema(returns));
            for field in properties(returns) {
                println!("      {field}");
            }
        }
    }
}

/// Render an object's JSON Schema as the field list it describes. A schema
/// printed raw is something an operator has to read past to see the two fields
/// it declares.
fn properties(schema: &Value) -> Vec<String> {
    let Some(properties) = schema.get("properties").and_then(Value::as_object) else {
        return Vec::new();
    };

    let required: Vec<&str> = schema
        .get("required")
        .and_then(Value::as_array)
        .map(|names| names.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();

    let width = properties.keys().map(String::len).max().unwrap_or(0);
    properties
        .iter()
        .map(|(name, schema)| {
            let kind = format::schema(schema);
            let required = if required.contains(&name.as_str()) {
                " (required)"
            } else {
                ""
            };
            let description = schema
                .get("description")
                .and_then(Value::as_str)
                .unwrap_or_default();
            format!("{name:<width$}  {kind:<8}{required:<11}  {description}")
        })
        .collect()
}
