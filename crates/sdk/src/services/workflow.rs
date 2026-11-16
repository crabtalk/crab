use crate::{services::price::Price, system::meta::Rates};
use berm_api::workflow::{Version, graph::Source, run::Accepted};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Resolved commercial terms, distinct from the execution checkpoint contract.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Terms {
    pub version: Version,
    pub source: Source,
    pub execution: Accepted,
    pub task_price: Option<Price>,
    /// Conservative direct-call and control quote in USD micros, not a transitive bound.
    pub direct_cost: u64,
    /// Cumulative managed charge cap in USD micros, across all attempts.
    pub max_cost: u64,
    pub rate_revision: String,
    pub rates: Rates,
    pub dependencies: BTreeMap<String, DependencyTerms>,
    /// Provider accounts charged independently of the managed cap.
    pub external_billing: Vec<String>,
}

#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyTerms {
    pub price: Option<Price>,
    pub allowance: u64,
}

/// Exact USD micros as decimal strings; unknown dispatches retain their reservations.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Spend {
    pub run: String,
    pub incurred: String,
    pub reserved: String,
}

#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum Settlement {
    Owner,
    Free,
    Held { reservation: String },
    Released,
    Settled { transaction: String },
}
