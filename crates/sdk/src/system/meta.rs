//! What `/meta` says about the deployment.

use crate::{
    account::billing::Unit,
    services::service::{MeteredDay, Resource},
};
use berm_api::Meter;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// CPU microseconds a run spends, over every call in it, before those past
/// them are charged as overage.
pub const INCLUDED_CPU: u64 = 100;

/// MiB-microseconds of memory held in a GB-hour.
pub const GB_HOUR: u64 = 1024 * 3_600_000_000;

/// `GET /meta/errors/{code}`: what an error code means and what to do about
/// it.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Explanation {
    /// As `E0001`.
    pub code: String,
    /// Markdown: a summary line, then the rest.
    pub text: String,
}

/// What is deployed, served unauthenticated at `GET /meta/version`.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionInfo {
    /// The running build's crate version, e.g. `0.1.1`, plain semver. It moves
    /// only when someone bumps it, not every deploy; `commit` identifies a
    /// build.
    pub version: String,
    /// The commit this binary was built from — the full 40-character SHA, e.g.
    /// `9bb4d4f11e60a4da6c8ca91a9dba2c5e874d2035`; truncate for display.
    /// `unknown` outside a git tree.
    pub commit: String,
    /// The database schema this deployment is on — the highest migration
    /// applied, matching the number a migration filename starts with. What the
    /// database holds, not merely what the binary ships.
    pub schema: i64,
}

/// `GET /meta/rates`: what metering costs a service's owner.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Metering {
    pub rates: Rates,
    /// CPU microseconds of each run `overage` does not count.
    pub included_cpu: u64,
    /// Bytes one log holds at most.
    pub max_log: usize,
}

/// What an owner is charged for its services' meters, each in USD micros per
/// billion units. Cloud's `[fleet.rates]` config table, and what it serves.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct Rates {
    pub runs: u64,
    /// CPU microseconds past each run's first [`INCLUDED_CPU`].
    pub overage: u64,
    pub read: u64,
    pub written: u64,
    /// MiB-microseconds of a call's memory budget held, waiting included.
    pub held: u64,
    /// Bytes through `berm.http.proxy`; unset is free.
    pub proxied: Option<u64>,
}

/// The resources charged by the unit, as their rates are named, in the order
/// [`Rates::metered`] takes them.
const UNITS: [&str; 6] = ["runs", "overage", "read", "written", "held", "proxied"];

/// The [`Unit`] a plan includes of each of [`UNITS`], in its order; proxied
/// bytes have none.
const INCLUDED: [Option<Unit>; 6] = [
    Some(Unit::Runs),
    Some(Unit::Cpu),
    Some(Unit::Read),
    Some(Unit::Written),
    Some(Unit::Held),
    None,
];

impl Rates {
    /// What `day` costs in USD micros, rounded down.
    pub fn cost(&self, day: &MeteredDay) -> i64 {
        self.price(day_units(day))
    }

    /// What `units` of [`UNITS`] cost in USD micros, rounded down.
    pub fn price(&self, units: [u64; 6]) -> i64 {
        (self.metered(units) / 1_000_000_000).min(i64::MAX as u128) as i64
    }

    /// What `units` of [`UNITS`] past a plan's included amounts cost, by the
    /// [`Unit`] each is included as, in USD micros, each rounded down; a unit
    /// costing nothing is left out.
    pub fn costs(&self, units: [u64; 6]) -> BTreeMap<Unit, i64> {
        INCLUDED
            .into_iter()
            .zip(units)
            .zip(self.rates())
            .filter_map(|((unit, units), rate)| {
                let micros = (units as u128 * rate as u128 / 1_000_000_000) as i64;
                (micros > 0).then_some((unit?, micros))
            })
            .collect()
    }

    /// What `meter` costs with `overage`, exactly, as a decimal of USD micros:
    /// `0.120000005`.
    pub fn cost_decimal(&self, meter: &Meter, overage: u64) -> String {
        decimal(self.metered(called(meter, overage)))
    }

    /// What `meter` costs with `overage`, in USD micros, rounded down.
    pub fn call_cost(&self, meter: &Meter, overage: u64) -> i64 {
        self.price(called(meter, overage))
    }

    /// Each charged resource over `days`: its units, its rate, and what they
    /// cost. `proxied` is left out while it is free.
    pub fn resources<'a>(&self, days: impl IntoIterator<Item = &'a MeteredDay>) -> Vec<Resource> {
        let mut counted = [0u64; 6];
        for day in days {
            for (sum, units) in counted.iter_mut().zip(day_units(day)) {
                *sum += units;
            }
        }
        UNITS
            .into_iter()
            .zip(counted)
            .zip(self.rates())
            .filter(|((resource, _), _)| *resource != "proxied" || self.proxied.is_some())
            .map(|((resource, units), rate)| Resource {
                resource: resource.to_owned(),
                units,
                rate,
                cost_micros_decimal: decimal(units as u128 * rate as u128),
            })
            .collect()
    }

    /// The rates of [`UNITS`], in its order.
    fn rates(&self) -> [u64; 6] {
        [
            self.runs,
            self.overage,
            self.read,
            self.written,
            self.held,
            self.proxied.unwrap_or(0),
        ]
    }

    /// What `units` of [`UNITS`] cost, in billionths of a USD micro.
    fn metered(&self, units: [u64; 6]) -> u128 {
        units
            .iter()
            .zip(self.rates())
            .map(|(units, rate)| *units as u128 * rate as u128)
            .sum()
    }
}

impl Default for Rates {
    fn default() -> Self {
        Self {
            runs: 150_000_000,
            overage: 20_000,
            read: 1_000_000,
            written: 1_000_000_000,
            held: 12,
            proxied: None,
        }
    }
}

/// `day`'s units of [`UNITS`], in its order.
fn day_units(day: &MeteredDay) -> [u64; 6] {
    [
        day.used.runs,
        day.overage,
        day.used.read,
        day.used.written,
        day.used.held,
        day.used.proxied,
    ]
}

/// `meter`'s units of [`UNITS`] with `overage`, in its order.
fn called(meter: &Meter, overage: u64) -> [u64; 6] {
    [
        meter.runs,
        overage,
        meter.read,
        meter.written,
        meter.held,
        meter.proxied,
    ]
}

/// `billionths` of a USD micro as a decimal of USD micros: `0.120000005`.
fn decimal(billionths: u128) -> String {
    let fraction = format!("{:09}", billionths % 1_000_000_000);
    match fraction.trim_end_matches('0') {
        "" => (billionths / 1_000_000_000).to_string(),
        fraction => format!("{}.{fraction}", billionths / 1_000_000_000),
    }
}
