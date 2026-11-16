use crate::{
    Access, Budget, Limits, ToolSpec, digest, schedule, schema,
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
    #[serde(default = "access", deserialize_with = "read_access")]
    pub access: Option<Access>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub example: Option<String>,
    #[serde(default)]
    pub price: Option<u64>,
    #[serde(default = "max_cost")]
    pub max_cost: u64,
    #[serde(default)]
    pub budget: Option<Limits>,
    #[serde(default = "checkpoint_bytes")]
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

impl Schedule {
    pub fn revision(&self) -> String {
        digest::json_hex(&serde_json::json!({"cron":self.cron,"input":self.input}))
    }

    pub fn check_input(&self, schema: &Value) -> Result<()> {
        schema::validate(schema, &self.input).map_err(|error| ScheduleInput(error).into())
    }
}

#[derive(Debug)]
pub struct ScheduleInput(pub anyhow::Error);
impl ScheduleInput {
    pub const EXPLAIN: &'static str = "E0074";
}
impl fmt::Display for ScheduleInput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid workflow schedule input: {}", self.0)
    }
}
impl std::error::Error for ScheduleInput {}

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
            ToolSpec::check_title(title)?;
        }
        ensure!(
            self.price.is_none() || self.access.is_some_and(|access| access != Access::Internal),
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
            schedule.check_input(&self.entry.graph.input)?;
        }
        Ok(())
    }

    pub fn public(&self) -> Option<Public> {
        Some(Public {
            entry: self.entry.clone(),
            title: self.config.title.clone(),
            access: self
                .config
                .access
                .filter(|access| *access != Access::Internal)?,
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
        if let Some(invalid) = error.downcast_ref::<Self>() {
            return Some(if invalid.0.is::<ScheduleInput>() {
                ScheduleInput::EXPLAIN
            } else {
                Self::EXPLAIN
            });
        }
        error
            .is::<ScheduleInput>()
            .then_some(ScheduleInput::EXPLAIN)
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

fn access() -> Option<Access> {
    Some(Access::Public)
}
fn max_cost() -> u64 {
    crate::DEFAULT_WORKFLOW_MAX_COST
}
fn checkpoint_bytes() -> u64 {
    crate::DEFAULT_WORKFLOW_CHECKPOINT_BYTES
}
impl Default for Config {
    fn default() -> Self {
        Self {
            access: access(),
            title: None,
            example: None,
            price: None,
            max_cost: max_cost(),
            budget: None,
            checkpoint_bytes: checkpoint_bytes(),
            retain: retention(),
            schedule: None,
        }
    }
}

fn read_access<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<Access>, D::Error> {
    Ok(Option::<Access>::deserialize(deserializer)?.filter(|access| *access != Access::Internal))
}
