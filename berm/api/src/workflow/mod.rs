use crate::Budget;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
pub mod config;
pub mod graph;
pub mod run;
pub mod schedule;
pub mod spend;
pub mod view;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Offer {
    pub source: graph::Source,
    pub spec: config::Spec,
    pub local_bindings: BTreeMap<String, run::Binding>,
    pub control_budget: Budget,
    pub control_buffer: u64,
    pub deadline_ms: u64,
}

/// Epoch timestamps and durations in this module use milliseconds.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "u16", into = "u16")]
pub enum Version {
    V1,
}

impl TryFrom<u16> for Version {
    type Error = String;

    fn try_from(version: u16) -> Result<Self, Self::Error> {
        match version {
            1 => Ok(Self::V1),
            _ => Err(format!("unsupported workflow contract version {version}")),
        }
    }
}

impl From<Version> for u16 {
    fn from(_: Version) -> Self {
        1
    }
}

#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Policy {
    pub retention_ms: u64,
    pub max_each: u32,
}

impl Default for Policy {
    fn default() -> Self {
        Self {
            retention_ms: 7 * 24 * 60 * 60 * 1000,
            max_each: 100,
        }
    }
}
