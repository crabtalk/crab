//! `crab inspect` — what a program claims to be.

use crate::{format, utils};
use anyhow::Result;
use berm_api::{Program, Runtime, ToolSpec};
use crabtalk_sdk::{
    client::{Client, Error},
    sandbox::service::{Measured, Terms},
};
use serde_json::Value;

/// Print `name` as its caller sees it: the runtime too when the host answers
/// it, which Cloud does for the service's owners alone.
pub fn run(client: &Client, name: &str, json: bool) -> Result<()> {
    let program = utils::wait(client.inspect(name))?;
    let runtime = match utils::wait(client.runtime(name)) {
        Ok(runtime) => Some(runtime),
        Err(Error::Api { status: 403, .. }) => None,
        Err(e) => return Err(e.into()),
    };
    let terms = utils::wait(client.terms(name))?;
    if json {
        return utils::json(&serde_json::json!({
            "program": program,
            "runtime": runtime,
            "terms": terms,
        }));
    }
    show(&program, runtime.as_ref(), &terms);
    Ok(())
}

pub fn show(program: &Program, runtime: Option<&Runtime>, terms: &Terms) {
    println!("{}", program.name);
    if !program.description.is_empty() {
        println!("  {}", program.description);
    }
    if !program.version.is_empty() {
        println!("  version {}", program.version);
    }
    println!("  digest  {}", program.digest);
    for (label, value) in [
        ("homepage", &program.homepage),
        ("repository", &program.repository),
    ] {
        if !value.is_empty() {
            println!("  {label} {value}");
        }
    }
    if !program.readme.is_empty() {
        println!();
        for line in program.readme.lines() {
            println!("  {line}");
        }
    }
    let Some(runtime) = runtime else {
        tools(program.tools.iter(), terms);
        return;
    };
    let answered = |missing: bool| match missing {
        true => "  (nothing answers to it here)",
        false => "",
    };
    for dep in &runtime.deps.programs {
        let missing = runtime.unresolved.programs.contains(dep);
        println!("  program {dep}{}", answered(missing));
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
        println!("  secret  {name}{}", answered(missing));
    }

    tools(program.tools.iter().chain(&runtime.tools), terms);
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

/// Each of `tools`, with its marks, arguments, reply, and what one call of it
/// is held to and costs.
fn tools<'a>(tools: impl Iterator<Item = &'a ToolSpec>, terms: &Terms) {
    for tool in tools {
        println!();
        let marks = [
            (tool.public, " (public)"),
            (tool.anonymous, " (anonymous)"),
            (tool.query, " (query)"),
            (tool.deprecated, " (deprecated)"),
        ];
        let marks: String = marks
            .iter()
            .filter(|(set, _)| *set)
            .map(|(_, mark)| *mark)
            .collect();
        println!("  {}{marks}", tool.name);
        println!("    {}", tool.description);
        if let Some(terms) = terms.tools.get(&tool.name) {
            let budget = terms.held.budget;
            let memory = match budget.memory {
                0 => "the host's most".to_owned(),
                bytes => format::bytes(bytes),
            };
            println!(
                "    at most {}µs of CPU, {} rows and {memory} of memory a call",
                budget.cpu, budget.rows
            );
            if let Some(price) = terms.price {
                println!("    {price} per call");
            }
            if let Some(estimate) = &terms.estimate {
                let cost = |m: &Measured| format::usd(m.cost_micros_decimal.parse().unwrap_or(0.0));
                println!(
                    "    {} to run typically, {} at p99, over {} runs",
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
