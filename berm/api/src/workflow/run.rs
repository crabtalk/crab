use crate::{
    Budget, Meter,
    host::Caller,
    workflow::{
        Version,
        graph::{Source, Target},
    },
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Run {
    pub version: Version,
    pub id: String,
    pub service: String,
    pub workflow: String,
    pub source: Source,
    pub input: Value,
    pub trigger: Trigger,
    pub created_at: u64,
    pub expires_at: u64,
    pub status: Status,
    pub accepted: Accepted,
    pub attempts: Vec<Attempt>,
    pub checkpoints: BTreeMap<String, Checkpoint>,
    pub result: Option<Value>,
    pub error: Option<Failure>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Trigger {
    Manual { caller: Caller },
    Schedule { key: String, at: u64 },
}

#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Accepted {
    /// Keyed by workflow node ID; all each items use the same binding.
    pub bindings: BTreeMap<String, Binding>,
    pub control_budget: Budget,
    pub control_buffer: u64,
    pub checkpoint_bytes: u64,
    /// Duration allowed for each attempt, including all its nested calls.
    pub deadline_ms: u64,
}

#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Binding {
    pub target: Target,
    pub service: String,
    pub image: String,
    pub input: Value,
    pub output: Value,
    pub budget: Budget,
    /// Revision of the authorization accepted before effects.
    pub authorization: String,
}

#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Pending,
    Running,
    Succeeded,
    Failed,
    Indeterminate,
    Cancelling,
    Cancelled,
}

#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attempt {
    pub id: String,
    pub started_at: u64,
    pub deadline_at: u64,
    pub finished_at: Option<u64>,
    pub status: Status,
    pub meter: Meter,
    pub error: Option<Failure>,
}

#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Checkpoint {
    pub state: State,
    pub prepared: Option<Prepared>,
    /// Ordered by original input index. Successful items survive continuation.
    pub items: Vec<Item>,
    pub meter: Meter,
}

#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum State {
    Pending,
    Prepared,
    Running,
    Succeeded { value: Value },
    Skipped,
    Failed { error: Failure },
    Unknown { error: Failure },
}

#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Prepared {
    Calls { calls: Vec<PreparedCall> },
    Branch { case: String, payload: Value },
    Output { value: Value },
}

#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PreparedCall {
    pub input: Value,
    /// Reused verbatim with the original input on eligible continuation.
    pub operation_key: Option<String>,
}

#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Item {
    pub index: u32,
    pub state: State,
    pub dispatches: Vec<Dispatch>,
}

#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dispatch {
    pub id: String,
    pub attempt: String,
    pub started_at: u64,
    pub finished_at: Option<u64>,
    pub state: State,
    pub meter: Meter,
}

/// Preserve the complete response body, including explanation and recovery fields.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Failure {
    pub status: u16,
    pub headers: BTreeMap<String, String>,
    pub body: Value,
}
