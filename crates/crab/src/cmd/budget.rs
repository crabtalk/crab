//! `crab budget` — the most one call of a tool may use, and what its runs used.

use crate::{format, runtime, terminal};
use anyhow::Result;
use berm_api::Budget;
use crabtalk_sdk::{client::Client, services::service::Measured};

/// Set `name`'s `tool` to at most `cpu` microseconds, `rows` rows and
/// `memory` MiB a call; `None` memory is the host's most.
pub fn set(
    client: &Client,
    name: &str,
    tool: &str,
    cpu: u64,
    rows: u64,
    memory: Option<u64>,
) -> Result<()> {
    runtime::wait(client.budget(
        name,
        tool,
        Budget {
            cpu,
            rows,
            memory: memory.map_or(0, |mib| mib << 20),
        },
    ))?;
    let memory = memory.map_or("the host's most".to_owned(), |mib| format!("{mib} MiB"));
    eprintln!("{name}.{tool}: at most {cpu}µs of CPU, {rows} rows and {memory} of memory a call");
    Ok(())
}

/// What `name`'s `tool` used over the last 30 days, its budget when it is
/// public, and the memory budget its runs fit.
pub fn show(client: &Client, name: &str, tool: &str, json: bool) -> Result<()> {
    let mut tools = runtime::wait(client.tools(name))?;
    let Some(used) = tools.remove(tool) else {
        println!("{name}.{tool} was not called in the last 30 days");
        return Ok(());
    };
    if json {
        return terminal::json(&used);
    }
    let terms = runtime::wait(client.terms(name))?;
    println!("{name}.{tool}");
    if let Some(terms) = terms.tools.get(tool) {
        let budget = terms.held.budget;
        let memory = match budget.memory {
            0 => "the host's most".to_owned(),
            bytes => format::bytes(bytes),
        };
        println!(
            "  budget       at most {}µs of CPU, {} rows and {memory} of memory a call",
            budget.cpu, budget.rows
        );
    }
    if let Some(estimate) = &used.estimate {
        println!(
            "  measured     {} runs that ended done",
            format::count(estimate.runs)
        );
        println!("  typical      {}", measured(&estimate.typical));
        println!("  slow (p99)   {}", measured(&estimate.slow));
    }
    if let Some(mib) = used.fits_mib {
        println!(
            "  memory       {} at most; --memory {mib} fits it at 1.5×",
            format::bytes(used.meter.memory)
        );
    }
    Ok(())
}

/// One measure of a run, on one line.
fn measured(m: &Measured) -> String {
    let cost = m.cost_micros_decimal.parse::<f64>().unwrap_or(0.0);
    format!(
        "{} ms, {}µs of CPU, {} rows read, {} written, {} to run",
        m.ms,
        m.cpu,
        format::count(m.read),
        format::count(m.written),
        format::usd(cost)
    )
}
