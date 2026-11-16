use crate::mcp;
use berm_api::workflow::view::Start;
use serde::Deserialize;
use serde_json::{Map, Value, json};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Inspect,
    Run,
    Status,
    Continue,
    Cancel,
}

impl Action {
    pub const ALL: [Self; 5] = [
        Self::Inspect,
        Self::Run,
        Self::Status,
        Self::Continue,
        Self::Cancel,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::Inspect => "workflow_inspect",
            Self::Run => "workflow_run",
            Self::Status => "workflow_status",
            Self::Continue => "workflow_continue",
            Self::Cancel => "workflow_cancel",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Self::Inspect => {
                "List accessible workflows when service is omitted. Supply service to list its workflows, or service and workflow for its graph, input/output schemas, task price and current admission quote. Inspection executes no application tools."
            }
            Self::Run => {
                "Admit a workflow using the inspected quote and input. Generate one UUID for request.id and reuse the identical request after transport failure. Returns the run identity and accepted terms; poll workflow_status. The fixed task price settles on success. Effects already performed are not rolled back on failure."
            }
            Self::Status => {
                "Read an authorized workflow run, checkpoints, result, expiry and continuation eligibility. Complete run data is restricted to its owner or initiator; owner accounting has a separate access check."
            }
            Self::Continue => {
                "Explicitly continue an eligible failed or indeterminate workflow using its original input and retained checkpoints. Reuses completed work, rechecks authorization and accepted bindings, and refuses unsafe unknown writes. Returns the same run identity."
            }
            Self::Cancel => {
                "Request cancellation of an authorized workflow run. Stops new dispatches; accepted external calls may finish and their effects are not undone."
            }
        }
    }

    pub fn read_only(self) -> bool {
        matches!(self, Self::Inspect | Self::Status)
    }

    pub fn schema(self) -> Map<String, Value> {
        let mut properties = mcp::object(json!({
            "service": {"type":"string", "description":"Service address returned by workflow_inspect; local hosts use an unqualified service name."},
            "workflow": {"type":"string", "description":"Workflow entry name returned by workflow_inspect."}
        }));
        let required = match self {
            Self::Inspect => Vec::new(),
            Self::Run => {
                properties.insert("request".into(), json!({
                    "type":"object", "additionalProperties":false,
                    "properties": {
                        "id":{"type":"string","format":"uuid","description":"Client-generated UUID, reused with identical arguments for transport retries."},
                        "quote":{"type":"string","description":"Current quote returned by workflow_inspect."},
                        "input":{"description":"Workflow input matching the inspected entry schema."}
                    }, "required":["id","quote","input"]
                }));
                vec!["service", "workflow", "request"]
            }
            _ => {
                properties.insert("id".into(), json!({"type":"string","format":"uuid","description":"Accepted workflow run identity."}));
                vec!["service", "workflow", "id"]
            }
        };
        mcp::object(
            json!({"type":"object","properties":properties,"required":required,"additionalProperties":false}),
        )
    }
}

impl TryFrom<&str> for Action {
    type Error = ();

    fn try_from(name: &str) -> Result<Self, Self::Error> {
        Self::ALL
            .into_iter()
            .find(|action| action.name() == name)
            .ok_or(())
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Inspect {
    pub service: Option<String>,
    pub workflow: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Run {
    pub service: String,
    pub workflow: String,
    pub request: Start,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Control {
    pub service: String,
    pub workflow: String,
    pub id: String,
}
