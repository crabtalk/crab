//! `crab usage` — what a program used, a day each; with no program, what the
//! account's services used and how much of its plan.

use crate::{format, utils};
use anyhow::Result;
use berm_api::host::Memory;
use crabtalk_sdk::{
    account::billing::{Period, Unit},
    client::Client,
};
use serde::Serialize;
use std::collections::BTreeMap;

pub fn run(client: &Client, name: &str, json: bool) -> Result<()> {
    let meters = utils::wait(client.meters(name))?;
    if json {
        return utils::json(&meters);
    }
    if meters.is_empty() {
        println!("nothing used in the last 30 days");
        return Ok(());
    }
    println!(
        "{:<10}  {:>8}  {:>8}  {:>8}  {:>14}  {:>14}  {:>10}  {:>10}  {:>12}  {:>10}  {:>13}  {:>12}",
        "DAY",
        "RUNS",
        "CALLS",
        "FAILED",
        "CPU µs",
        "OVERAGE",
        "READ",
        "WRITTEN",
        "SQL CPU µs",
        "MEMORY",
        "PROXIED BYTES",
        "STORED BYTES"
    );
    for day in &meters {
        println!(
            "{:<10}  {:>8}  {:>8}  {:>8}  {:>14}  {:>14}  {:>10}  {:>10}  {:>12}  {:>8}Ki  {:>13}  {:>12}",
            format::day(day.used.day.into()),
            day.used.runs,
            day.used.calls,
            day.used.failed,
            day.used.cpu,
            day.overage,
            day.used.read,
            day.used.written,
            day.used.sql_cpu,
            day.used.memory / 1024,
            day.used.proxied,
            day.used.stored,
        );
    }
    Ok(())
}

/// One service's sum over the 30 days.
#[derive(Default, Serialize)]
struct Sum {
    runs: u64,
    calls: u64,
    failed: u64,
    cost_micros: i64,
}

/// The account's services over the last 30 days, and its plan this period.
pub fn account(client: &Client, json: bool) -> Result<()> {
    let usage = utils::wait(client.services_usage())?;
    let period = utils::wait(client.period())?;
    let memory = utils::wait(client.memory())?;
    let services: BTreeMap<String, Sum> = usage
        .into_iter()
        .map(|(name, days)| {
            let sum = days.iter().fold(Sum::default(), |sum, day| Sum {
                runs: sum.runs + day.used.used.runs,
                calls: sum.calls + day.used.used.calls,
                failed: sum.failed + day.used.used.failed,
                cost_micros: sum.cost_micros + day.cost_micros,
            });
            (name, sum)
        })
        .collect();
    if json {
        #[derive(Serialize)]
        struct Account<'a> {
            services: &'a BTreeMap<String, Sum>,
            period: &'a Period,
            memory: &'a Memory,
        }
        return utils::json(&Account {
            services: &services,
            period: &period,
            memory: &memory,
        });
    }

    let width = services.keys().map(String::len).max().unwrap_or(0).max(7);
    println!(
        "{:<width$}  {:>10}  {:>10}  {:>10}  {:>6}  {:>10}",
        "SERVICE", "RUNS", "CALLS", "FAILED", "", "COST"
    );
    for (name, sum) in &services {
        println!(
            "{name:<width$}  {:>10}  {:>10}  {:>10}  {:>6}  {:>10}",
            format::count(sum.runs),
            format::count(sum.calls),
            format::count(sum.failed),
            failed(sum),
            format::usd(sum.cost_micros as f64)
        );
    }
    println!("over the last 30 days, at the rates, before included amounts");
    println!();
    println!(
        "plan {}, {} through {}",
        period.plan,
        format::day(period.since.into()),
        format::day(i64::from(period.until) - 1)
    );
    // A unit's label, what was used of it and what is included, how it reads,
    // and the unit its cost past the included amount is kept under.
    type Line = (&'static str, u64, u64, fn(u64) -> String, Unit);
    let included: [Line; 6] = [
        (
            "runs",
            period.used.runs,
            period.included.runs,
            format::count,
            Unit::Runs,
        ),
        (
            "CPU µs",
            period.used.cpu,
            period.included.cpu,
            format::count,
            Unit::Cpu,
        ),
        (
            "rows read",
            period.used.read,
            period.included.read,
            format::count,
            Unit::Read,
        ),
        (
            "rows written",
            period.used.written,
            period.included.written,
            format::count,
            Unit::Written,
        ),
        (
            "memory held",
            period.used.held,
            period.included.held,
            format::gb_hours,
            Unit::Held,
        ),
        (
            "storage",
            period.used.stored,
            period.included.stored,
            format::bytes,
            Unit::Stored,
        ),
    ];
    let mut any = false;
    for (label, used, of, size, unit) in included.into_iter().filter(|unit| unit.2 > 0) {
        any = true;
        println!(
            "  {label:<14}  {:>10} of {:<10}  {:>5}  {}",
            size(used),
            size(of),
            format::percent(used as f64 / of as f64),
            format::usd(period.costs.get(&unit).copied().unwrap_or(0) as f64)
        );
    }
    if any {
        println!(
            "  past the included amounts  {}",
            format::usd(period.charge as f64)
        );
    }
    if !any {
        println!("  includes no amounts: it runs no services of its own");
    }
    for warning in &period.warnings {
        println!("  warned: {} at {}%", warning.unit, warning.threshold);
    }
    if !period.spent.is_empty() {
        println!();
        let width = period
            .spent
            .iter()
            .map(|spent| spent.owner.len() + spent.service.len() + spent.tool.len() + 2)
            .max()
            .unwrap_or(0)
            .max(4);
        println!("prices paid this period");
        println!("  {:<width$}  {:>10}  {:>10}", "TOOL", "CALLS", "SPENT");
        for spent in &period.spent {
            println!(
                "  {:<width$}  {:>10}  {:>10}",
                format!("{}/{}.{}", spent.owner, spent.service, spent.tool),
                format::count(spent.calls),
                format::usd(spent.micros as f64)
            );
        }
    }
    println!();
    let held = memory.loaded + memory.running;
    let limit = match memory.limit {
        0 => "no limit".to_owned(),
        limit => format!(
            "of {}  {}",
            format::bytes(limit),
            format::percent(held as f64 / limit as f64)
        ),
    };
    println!(
        "memory {} held {limit}: {} loaded, {} running",
        format::bytes(held),
        format::bytes(memory.loaded),
        format::bytes(memory.running)
    );
    Ok(())
}

/// The share of `sum`'s calls that failed.
fn failed(sum: &Sum) -> String {
    match sum.calls {
        0 => String::new(),
        calls => format::percent(sum.failed as f64 / calls as f64),
    }
}
