//! `crab budget` — the most one call of a tool may use, and what its runs used.

use crate::{format, runtime, terminal};
use anyhow::Result;
use berm_api::Budget;
use crabtalk_sdk::{
    client::Client,
    services::service::{Estimate, Measured},
};

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
/// public with what one run costs at most when `cloud` answers with its rates,
/// and the memory budget its runs fit.
pub fn show(client: &Client, cloud: &Client, name: &str, tool: &str, json: bool) -> Result<()> {
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
    if let Some(held) = terms.tools.get(tool) {
        let budget = held.held.budget;
        println!(
            "  budget       at most {}µs of CPU, {} rows and {} of memory a call",
            budget.cpu,
            budget.rows,
            format::bytes(budget.memory)
        );
        if let Ok(metering) = runtime::wait(cloud.rates()) {
            let most = metering.most(&budget, terms.deadline);
            println!(
                "  most         {} to run, its calls to other services aside",
                format::usd(most.parse().unwrap_or(0.0))
            );
        }
    }
    if let Some(estimate) = &used.estimate {
        println!(
            "  measured     {} runs that ended done",
            format::count(estimate.runs)
        );
        println!("  typical      {}", measured(&estimate.typical));
        let slow = if estimate.runs >= Estimate::P99 {
            "slow (p99)"
        } else {
            "slowest"
        };
        println!("  {slow:<12} {}", measured(&estimate.slow));
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
        "{} ms, {}µs of CPU, {} rows read, {} written, {:.1} MiB-s held, {} to run",
        m.ms,
        m.cpu,
        format::count(m.read),
        format::count(m.written),
        m.held as f64 / 1e6,
        format::usd(cost)
    )
}
