//! `crab inspect` — what a service claims to be.

use crate::{format, runtime, terminal};
use anyhow::Result;
use berm_api::{Access, Runtime, Service, ToolSpec};
use crabtalk_sdk::{
    client::{Client, Error},
    services::service::{Estimate, Measured, Terms},
    system::meta::Metering,
};
use serde_json::Value;

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
    show(&service, runtime.as_ref(), &terms, metering.as_ref());
    Ok(())
}

pub fn show(
    service: &Service,
    runtime: Option<&Runtime>,
    terms: &Terms,
    metering: Option<&Metering>,
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
    let Some(runtime) = runtime else {
        tools(service.tools.iter(), terms, metering);
        return;
    };
    let answered = |missing: bool| match missing {
        true => "  (nothing answers to it here)",
        false => "",
    };
    for dep in &runtime.deps.calls {
        let missing = runtime.unresolved.calls.contains(dep);
        println!("  service {dep}{}", answered(missing));
    }
    for (name, reached) in &runtime.deps.syscalls {
        let missing = runtime.unresolved.syscalls.contains_key(name);
        let reached = match reached.is_empty() {
            true => String::new(),
            false => format!(" -> {}", reached.join(", ")),
        };
        println!("  syscall {name}{reached}{}", answered(missing));
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
                (tool.deprecated, "deprecated"),
            ]
            .into_iter()
            .filter_map(|(set, mark)| set.then_some(mark))
            .collect();
            let price = terms
                .tools
                .get(&tool.name)
                .and_then(|terms| terms.price)
                .map(|price| price.to_string());
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
                format::bytes(budget.memory)
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
        for parameter in properties(&tool.parameters) {
            println!("    {parameter}");
        }
        if let Some(returns) = &tool.returns {
            println!("    returns");
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
            let kind = schema.get("type").and_then(Value::as_str).unwrap_or("any");
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
