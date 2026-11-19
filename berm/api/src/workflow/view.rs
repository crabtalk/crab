use crate::{
    host,
    workflow::{
        Version,
        graph::{Entry, Source},
        run::{self, Failure, Status},
    },
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[cfg_attr(feature = "utoipa", schema(as = WorkflowDefinition))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Inspection {
    pub version: Version,
    pub service: String,
    pub entry: Entry,
    pub title: Option<String>,
    pub source: Source,
    pub quote: String,
    pub task_price: Option<u64>,
    /// The most a successful run costs the caller in USD micros: the task
    /// price plus each priced step's price times its most items. Absent where
    /// runs are not charged.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_price: Option<u64>,
    pub deadline_ms: u64,
    pub retention_ms: u64,
}

#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[cfg_attr(feature = "utoipa", schema(as = WorkflowStart))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Start {
    /// Client-generated UUID; transport retries reuse this identity and identical arguments.
    pub id: String,
    pub quote: String,
    pub input: Value,
}

#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[cfg_attr(feature = "utoipa", schema(as = WorkflowExecution))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Run {
    pub version: Version,
    pub id: String,
    pub service: String,
    pub workflow: String,
    pub source: Source,
    pub input: Value,
    pub created_at: u64,
    pub expires_at: u64,
    pub status: Status,
    pub attempts: Vec<Attempt>,
    pub checkpoints: BTreeMap<String, Checkpoint>,
    pub result: Option<Value>,
    pub error: Option<Failure>,
    pub continuation: Continuation,
}

#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[cfg_attr(feature = "utoipa", schema(as = WorkflowContinuation))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum Continuation {
    Available,
    Unavailable { reason: Failure },
}

#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[cfg_attr(feature = "utoipa", schema(as = WorkflowAttempt))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attempt {
    pub id: String,
    pub started_at: u64,
    pub deadline_at: u64,
    pub finished_at: Option<u64>,
    pub status: Status,
    pub error: Option<Failure>,
}

#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[cfg_attr(feature = "utoipa", schema(as = WorkflowCheckpoint))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Checkpoint {
    pub state: run::State,
    pub items: Vec<Item>,
}

#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[cfg_attr(feature = "utoipa", schema(as = WorkflowItem))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Item {
    pub index: u32,
    pub state: run::State,
    pub dispatches: Vec<Dispatch>,
}

#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[cfg_attr(feature = "utoipa", schema(as = WorkflowDispatch))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dispatch {
    pub id: String,
    pub attempt: String,
    pub started_at: u64,
    pub finished_at: Option<u64>,
    pub state: run::State,
}

impl Run {
    pub fn from_native(run: &run::Run, continuation: Continuation) -> Self {
        let mut view = Self {
            version: run.version,
            id: run.id.clone(),
            service: run.service.clone(),
            workflow: run.workflow.clone(),
            source: run.source.clone(),
            input: run.input.clone(),
            created_at: run.created_at,
            expires_at: run.expires_at,
            status: run.status,
            attempts: run
                .attempts
                .iter()
                .map(|attempt| Attempt {
                    id: attempt.id.clone(),
                    started_at: attempt.started_at,
                    deadline_at: attempt.deadline_at,
                    finished_at: attempt.finished_at,
                    status: attempt.status,
                    error: attempt.error.clone(),
                })
                .collect(),
            checkpoints: run
                .checkpoints
                .iter()
                .map(|(id, checkpoint)| {
                    (
                        id.clone(),
                        Checkpoint {
                            state: checkpoint.state.clone(),
                            items: checkpoint
                                .items
                                .iter()
                                .map(|item| Item {
                                    index: item.index,
                                    state: item.state.clone(),
                                    dispatches: item
                                        .dispatches
                                        .iter()
                                        .map(|dispatch| Dispatch {
                                            id: dispatch.id.clone(),
                                            attempt: dispatch.attempt.clone(),
                                            started_at: dispatch.started_at,
                                            finished_at: dispatch.finished_at,
                                            state: dispatch.state.clone(),
                                        })
                                        .collect(),
                                })
                                .collect(),
                        },
                    )
                })
                .collect(),
            result: run.result.clone(),
            error: run.error.clone(),
            continuation,
        };
        view.remove_error_headers(&[host::METER, host::CALLER]);
        view
    }

    pub fn remove_error_headers(&mut self, names: &[&str]) {
        let remove = |failure: &mut Failure| {
            failure.headers.retain(|name, _| {
                !names
                    .iter()
                    .any(|private| name.eq_ignore_ascii_case(private))
            });
        };
        let state = |state: &mut run::State| {
            if let run::State::Failed { error } | run::State::Unknown { error } = state {
                remove(error);
            }
        };
        if let Some(error) = &mut self.error {
            remove(error);
        }
        if let Continuation::Unavailable { reason } = &mut self.continuation {
            remove(reason);
        }
        for attempt in &mut self.attempts {
            if let Some(error) = &mut attempt.error {
                remove(error);
            }
        }
        for checkpoint in self.checkpoints.values_mut() {
            state(&mut checkpoint.state);
            for item in &mut checkpoint.items {
                state(&mut item.state);
                for dispatch in &mut item.dispatches {
                    state(&mut dispatch.state);
                }
            }
        }
    }
}

#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[cfg_attr(feature = "utoipa", schema(as = LocalWorkflowRun))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Record {
    #[serde(flatten)]
    pub execution: Run,
    pub accepted: Inspection,
}
