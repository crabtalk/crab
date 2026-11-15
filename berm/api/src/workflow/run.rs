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
use std::{collections::BTreeMap, fmt};

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
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub query: bool,
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Item {
    pub index: u32,
    pub state: State,
    pub dispatches: Vec<Dispatch>,
}

#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

impl Checkpoint {
    pub fn advance(&mut self, next: Self) -> Result<(), String> {
        if matches!(self.state, State::Succeeded { .. } | State::Skipped) && *self != next {
            return Err("completed checkpoints cannot change".into());
        }
        if self.prepared.is_some() && self.prepared != next.prepared {
            return Err("prepared payloads and operation keys cannot change".into());
        }
        if next.items.len() < self.items.len() {
            return Err("checkpoint items cannot be truncated".into());
        }
        for (index, item) in next.items.iter().enumerate() {
            if item.index as usize != index {
                return Err("checkpoint items must retain their original order".into());
            }
            if let Some(previous) = self.items.get(index) {
                if matches!(previous.state, State::Succeeded { .. }) && previous != item {
                    return Err("successful items cannot change".into());
                }
                if item.dispatches.len() < previous.dispatches.len() {
                    return Err("dispatch history cannot be truncated".into());
                }
                for (old, new) in previous.dispatches.iter().zip(&item.dispatches) {
                    if old.id != new.id
                        || old.attempt != new.attempt
                        || old.started_at != new.started_at
                    {
                        return Err("dispatch identity cannot change".into());
                    }
                    if !matches!(old.state, State::Running) && old != new {
                        return Err("completed dispatches cannot change".into());
                    }
                }
            }
        }
        if let Some(Prepared::Calls { calls }) = &next.prepared
            && next.items.len() > calls.len()
        {
            return Err("checkpoint items exceed the prepared call list".into());
        }
        *self = next;
        Ok(())
    }
}

impl Run {
    pub fn interrupt(&mut self, error: Failure, now: u64) {
        let cancelled = self.status == Status::Cancelling;
        self.status = if cancelled {
            Status::Cancelled
        } else {
            Status::Indeterminate
        };
        self.error = Some(error.clone());
        if let Some(attempt) = self.attempts.last_mut() {
            attempt.status = self.status;
            attempt.finished_at = Some(now);
            attempt.error = Some(error.clone());
        }
        for checkpoint in self.checkpoints.values_mut() {
            if checkpoint.state == State::Running {
                checkpoint.state = State::Unknown {
                    error: error.clone(),
                };
            }
            for item in &mut checkpoint.items {
                if item.state == State::Running {
                    item.state = State::Unknown {
                        error: error.clone(),
                    };
                }
                for dispatch in &mut item.dispatches {
                    if dispatch.state == State::Running {
                        dispatch.state = State::Unknown {
                            error: error.clone(),
                        };
                        dispatch.finished_at = Some(now);
                    }
                }
            }
        }
    }
}

impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.body.fmt(f)
    }
}
impl std::error::Error for Failure {}

impl Binding {
    pub const CHANGED: &'static str = "E0060";
}
