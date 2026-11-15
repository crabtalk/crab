use crate::{
    Access, Budget, Limits, ToolSpec, schedule, schema,
    workflow::{
        Policy,
        graph::{Entry, Kind},
    },
};
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{fmt, time::Duration};

#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(default)]
    pub access: Option<Access>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub price: Option<u64>,
    pub max_cost: u64,
    #[serde(default)]
    pub budget: Option<Limits>,
    pub checkpoint_bytes: u64,
    #[serde(default = "retention", with = "humantime_serde")]
    #[cfg_attr(feature = "utoipa", schema(value_type = String))]
    pub retain: Duration,
    #[serde(default)]
    pub schedule: Option<Schedule>,
}

#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Schedule {
    pub cron: String,
    pub input: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Spec {
    pub entry: Entry,
    pub config: Config,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Public {
    pub entry: Entry,
    pub title: Option<String>,
    pub access: Access,
    pub price: Option<u64>,
}

impl Config {
    pub fn control_budget(&self, default: Budget) -> Budget {
        let budget = self.budget.unwrap_or_default().over(default);
        Budget {
            read: 0,
            written: 0,
            ..budget
        }
    }

    pub fn check(&self) -> Result<()> {
        ensure!(
            self.checkpoint_bytes > 0,
            "checkpoint_bytes must be positive"
        );
        ensure!(
            !self.retain.is_zero() && self.retain.as_millis() <= u128::from(u64::MAX),
            "retention must fit a positive millisecond duration"
        );
        if let Some(title) = &self.title {
            ensure!(
                title.chars().count() <= ToolSpec::TITLE && !title.contains(['\n', '\r']),
                "a workflow title is one line of at most {} characters",
                ToolSpec::TITLE
            );
        }
        ensure!(
            self.price.is_none() || self.access.is_some(),
            "a priced workflow needs caller access"
        );
        if let Some(budget) = self.budget {
            ensure!(
                budget.cpu != Some(0) && budget.memory != Some(0),
                "control CPU and memory budgets must be positive"
            );
            ensure!(
                budget.read.unwrap_or(0) == 0 && budget.written.unwrap_or(0) == 0,
                "workflow controls cannot read or write application storage"
            );
        }
        if let Some(schedule) = &self.schedule {
            schedule::parse(&schedule.cron)?;
        }
        Ok(())
    }
}

impl Spec {
    pub fn check(&self) -> Result<()> {
        self.config.check()?;
        schema::compile(&self.entry.graph.input)?;
        schema::compile(&self.entry.graph.output)?;
        for node in &self.entry.graph.nodes {
            match &node.kind {
                Kind::Step { call, .. } => {
                    schema::compile(&call.input)?;
                    schema::compile(&call.output)?;
                    for case in &call.cases {
                        schema::compile(&case.schema)?;
                    }
                }
                Kind::Branch { cases } => {
                    ensure!(!cases.is_empty(), "a branch needs at least one case");
                    for case in cases {
                        schema::compile(&case.schema)?;
                    }
                }
                Kind::Output => {}
            }
        }
        self.entry.graph.check(Policy::default().max_each)?;
        if let Some(schedule) = &self.config.schedule {
            schema::validate(&self.entry.graph.input, &schedule.input)?;
        }
        Ok(())
    }

    pub fn public(&self) -> Option<Public> {
        Some(Public {
            entry: self.entry.clone(),
            title: self.config.title.clone(),
            access: self.config.access?,
            price: self.config.price,
        })
    }
}

fn retention() -> Duration {
    Duration::from_millis(Policy::default().retention_ms)
}

#[derive(Debug)]
pub struct Invalid(pub anyhow::Error);

impl Invalid {
    pub fn explanation(error: &anyhow::Error) -> Option<&'static str> {
        error.is::<Self>().then_some(Self::EXPLAIN)
    }
    pub const EXPLAIN: &'static str = "E0058";
}

impl From<anyhow::Error> for Invalid {
    fn from(error: anyhow::Error) -> Self {
        Self(error)
    }
}
impl fmt::Display for Invalid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}
impl std::error::Error for Invalid {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.0.source()
    }
}
