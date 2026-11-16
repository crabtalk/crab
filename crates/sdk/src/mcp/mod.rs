//! Shared MCP contracts for service discovery, inspection and calls.

use berm_api::{Service, ToolSpec};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;

pub mod workflow;

pub const SEARCH: &str = "search";
pub const INSPECT: &str = "inspect";
pub const CALL: &str = "call";
pub const QUERY: &str = "query";
pub const SERVICES: &str = "services";
pub const FOUND: usize = 10;
pub const QUERY_EXPLAIN: &str = "E0077";
pub const CHANGED_EXPLAIN: &str = "E0078";

/// The resource template naming a service's README.
pub const TEMPLATE: &str = "crab://{+service}";

const SCHEME: &str = "crab://";
pub const REQUEST_SCHEME: &str = "crab-request://";
pub const REQUEST_TEMPLATE: &str = "crab-request://{code}";

pub const INSTRUCTIONS: &str = "Use `services` for your account's and organizations' services, or `search` for public services. \
    Use `inspect` on a service address to read its README, input schemas and prices before calling. \
    Run a tool marked query with `query`; use `call` for other tools and respect client confirmations. \
    Both take service, tool and input. Paid Cloud calls require the key's service grant and budget. \
    Relay operations use this same path. Use `workflow_inspect` to discover workflow graphs and quotes, \
    `workflow_run` to admit a run, and `workflow_status` to poll it. Continuation is explicit through `workflow_continue`; `workflow_cancel` cannot undo accepted effects.";

pub const SEARCH_DESCRIPTION: &str = "Find public services and tools by text; local hosts search running services. Returns service addresses, descriptions and tool names and titles, without input schemas. Inspect a result before calling it.";
pub const SERVICES_DESCRIPTION: &str = "List the connected account's and organizations' services, or services on a local host. Inspect a service to read its tools, input schemas and prices.";
pub const INSPECT_DESCRIPTION: &str = "Read one service's README, tools, input schemas and prices in USD micros. Supply an address returned by services or search, or an address the user supplied.";
pub const QUERY_DESCRIPTION: &str = "Run a service tool marked query. Refuses other tools and names call as the alternative. Supply service, tool and input from inspect; paid calls use the key's grants and budget.";
pub const CALL_DESCRIPTION: &str = "Run a service tool with possible side effects. Supply service, tool and input from inspect; respect the client's confirmation and the key's grants and budget.";
pub const TEMPLATE_DESCRIPTION: &str =
    "A service's README. Use inspect for tool schemas and prices.";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Search,
    Services,
    Inspect,
    Query,
    Call,
}

impl Action {
    pub const ALL: [Self; 5] = [
        Self::Search,
        Self::Services,
        Self::Inspect,
        Self::Query,
        Self::Call,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Self::Search => SEARCH,
            Self::Services => SERVICES,
            Self::Inspect => INSPECT,
            Self::Query => QUERY,
            Self::Call => CALL,
        }
    }
    pub fn description(self) -> &'static str {
        match self {
            Self::Search => SEARCH_DESCRIPTION,
            Self::Services => SERVICES_DESCRIPTION,
            Self::Inspect => INSPECT_DESCRIPTION,
            Self::Query => QUERY_DESCRIPTION,
            Self::Call => CALL_DESCRIPTION,
        }
    }
    pub fn read_only(self) -> bool {
        self != Self::Call
    }
    pub fn schema(self) -> Map<String, Value> {
        match self {
            Self::Search => search_schema(),
            Self::Services => object(json!({"type":"object", "properties":{}})),
            Self::Inspect => inspect_schema(),
            Self::Query | Self::Call => call_schema(),
        }
    }
    pub fn check_tool(self, tool: &ToolSpec) -> Result<(), String> {
        if self == Self::Query && !tool.query {
            Err(format!(
                "tool {:?} is not marked query; use call",
                tool.name
            ))
        } else {
            Ok(())
        }
    }
}
impl TryFrom<&str> for Action {
    type Error = String;
    fn try_from(name: &str) -> Result<Self, Self::Error> {
        Self::ALL
            .into_iter()
            .find(|action| action.name() == name)
            .ok_or_else(|| format!("no tool named {name:?}"))
    }
}

/// `search`'s input schema.
pub fn search_schema() -> Map<String, Value> {
    object(json!({
        "type": "object",
        "properties": {
            "query": {
                "type": "string",
                "description": "Words describing what a tool does.",
            },
        },
        "required": ["query"],
    }))
}

pub fn inspect_schema() -> Map<String, Value> {
    object(json!({
        "type": "object",
        "properties": {
            "service": {
                "type": "string",
                "description": "The service address from services or search; local hosts use an unqualified name.",
            },
        },
        "required": ["service"],
    }))
}

/// `call`'s input schema.
pub fn call_schema() -> Map<String, Value> {
    object(json!({
        "type": "object",
        "properties": {
            "service": {
                "type": "string",
                "description": "The service address from services or search; local hosts use an unqualified name.",
            },
            "tool": {
                "type": "string",
                "description": "The tool, as the service's IDL names it.",
            },
            "input": {
                "type": "object",
                "description": "The tool's arguments, as its schema in the IDL lays out.",
            },
        },
        "required": ["service", "tool"],
    }))
}

fn object(value: Value) -> Map<String, Value> {
    match value {
        Value::Object(map) => map,
        _ => Map::new(),
    }
}

/// The service a resource URI names.
pub fn service(uri: &str) -> Option<&str> {
    uri.strip_prefix(SCHEME)
        .filter(|service| !service.is_empty())
}

/// The service-request code a resource URI names.
pub fn request(uri: &str) -> Option<&str> {
    uri.strip_prefix(REQUEST_SCHEME).filter(|code| {
        !code.is_empty()
            && code
                .bytes()
                .all(|byte| byte.is_ascii_alphabetic() || byte == b'-')
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Inspection {
    pub service: String,
    pub definition: Service,
    /// USD micros per call, keyed by tool name; omitted entries are free.
    pub prices: BTreeMap<String, i64>,
}

/// A service `search` found.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Found {
    /// What `call` and the resource name it by.
    pub service: String,
    pub description: String,
    pub tools: Vec<Tool>,
}

/// A tool of a [`Found`] service.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tool {
    pub name: String,
    /// Its display name, MCP's tool `title`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
}
