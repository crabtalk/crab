//! `crab usage` — what a service used, a day each; with no service, what the
//! account's services used and how much of its plan.

use crate::{format, runtime, terminal};
use anyhow::Result;
use berm_api::host::Memory;
use crabtalk_sdk::{
    account::billing::{Period, Unit},
    client::Client,
    format as display,
};
use serde::Serialize;
use std::collections::BTreeMap;

pub fn run(client: &Client, name: &str, json: bool) -> Result<()> {
    let meters = runtime::wait(client.meters(name))?;
    if json {
        return terminal::json(&meters);
    }
    if meters.is_empty() {
        println!("nothing used in the last 30 days");
        return Ok(());
    }
    println!(
        "{:<10}  {:>8}  {:>8}  {:>8}  {:>14}  {:>14}  {:>10}  {:>10}  {:>12}  {:>10}  {:>12}  {:>12}",
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
        "DATABASE",
        "ROTATING"
    );
    for day in &meters {
        println!(
            "{:<10}  {:>8}  {:>8}  {:>8}  {:>14}  {:>14}  {:>10}  {:>10}  {:>12}  {:>10}  {:>12}  {:>12}",
            format::day(day.used.day.into()),
            day.used.runs,
            day.used.calls,
            day.used.failed,
            day.used.cpu,
            day.overage,
            day.used.read,
            day.used.written,
            day.used.sql_cpu,
            display::bytes(day.used.memory),
            day.used
                .database
                .map(|sample| display::bytes(sample.used))
                .unwrap_or_else(|| "—".into()),
            display::decimal_bytes(day.used.proxied),
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
    rotating_bytes: u64,
}

/// The account's services over the last 30 days, and its plan this period.
pub fn account(client: &Client, json: bool) -> Result<()> {
    let (usage, period, memory) = runtime::wait(async {
        tokio::try_join!(client.services_usage(), client.period(), client.memory())
    })?;
    let services: BTreeMap<String, Sum> = usage
        .into_iter()
        .map(|(name, days)| {
            let sum = days.iter().fold(Sum::default(), |sum, day| Sum {
                runs: sum.runs + day.used.used.runs,
                calls: sum.calls + day.used.used.calls,
                failed: sum.failed + day.used.used.failed,
                cost_micros: sum.cost_micros + day.cost_micros,
                rotating_bytes: sum.rotating_bytes + day.used.used.proxied,
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
        return terminal::json(&Account {
            services: &services,
            period: &period,
            memory: &memory,
        });
    }

    let width = services.keys().map(String::len).max().unwrap_or(0).max(7);
    println!(
        "{:<width$}  {:>10}  {:>10}  {:>10}  {:>6}  {:>10}  {:>12}",
        "SERVICE", "RUNS", "CALLS", "FAILED", "", "COST", "ROTATING"
    );
    for (name, sum) in &services {
        println!(
            "{name:<width$}  {:>10}  {:>10}  {:>10}  {:>6}  {:>10}  {:>12}",
            format::count(sum.runs),
            format::count(sum.calls),
            format::count(sum.failed),
            failed(sum),
            format::usd(sum.cost_micros as f64),
            display::decimal_bytes(sum.rotating_bytes)
        );
    }
    println!("over the last 30 days, at the rates");
    println!();
    println!(
        "plan {}, {} through {}",
        period.plan,
        format::day(period.since.into()),
        format::day(i64::from(period.until) - 1)
    );
    // A unit's label, what was used of it, how it reads, and the unit its cost
    // is kept under.
    type Line = (&'static str, u64, fn(u64) -> String, Unit);
    let compute: [Line; 5] = [
        ("runs", period.used.runs, format::count, Unit::Runs),
        ("CPU µs", period.used.cpu, format::count, Unit::Cpu),
        ("rows read", period.used.read, format::count, Unit::Read),
        (
            "rows written",
            period.used.written,
            format::count,
            Unit::Written,
        ),
        (
            "memory held",
            period.used.held,
            format::gb_hours,
            Unit::Held,
        ),
    ];
    for (label, used, size, unit) in compute {
        println!(
            "  {label:<14}  {:>10}  {}",
            size(used),
            format::usd(period.costs.get(&unit).copied().unwrap_or(0) as f64)
        );
    }
    println!(
        "  compute        {:>10}  paid from credit",
        format::usd(period.charge as f64)
    );
    let limits = [
        ("storage", period.used.stored, period.included.stored),
        ("outbound data", period.used.sent, period.included.sent),
    ];
    for (label, used, of) in limits.into_iter().filter(|limit| limit.2 > 0) {
        println!(
            "  {label:<14}  {:>10} of {:<10}  {:>5}",
            display::bytes(used),
            display::bytes(of),
            format::percent(used as f64 / of as f64)
        );
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
            display::bytes(limit),
            format::percent(held as f64 / limit as f64)
        ),
    };
    println!(
        "memory {} held {limit}: {} loaded, {} running",
        display::bytes(held),
        display::bytes(memory.loaded),
        display::bytes(memory.running)
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
