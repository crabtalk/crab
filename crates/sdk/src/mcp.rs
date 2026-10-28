//! The MCP surface Cloud and the local host both serve: fixed `search` and
//! `call` tools, and each service's README and IDL as the resource
//! `crab://{service}`.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

pub const SEARCH: &str = "search";
pub const CALL: &str = "call";

/// The resource template naming a service's README and IDL.
pub const TEMPLATE: &str = "crab://{+service}";

const SCHEME: &str = "crab://";

pub const INSTRUCTIONS: &str = "Find services with `search`, read one's resource \
    `crab://{service}` for its README and its tools' schemas, then run a tool with `call`.";

pub const SEARCH_DESCRIPTION: &str = "Find services whose name, description or tools match a \
    query, best first, each with the tools it can be called on and their prices in USD micros.";

pub const CALL_DESCRIPTION: &str = "Call a service's tool. A tool with a price charges it to \
    the calling account's credit; a call that fails is returned.";

pub const TEMPLATE_DESCRIPTION: &str =
    "A service's README, then its IDL: each tool's description and schema.";

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

/// `call`'s input schema.
pub fn call_schema() -> Map<String, Value> {
    object(json!({
        "type": "object",
        "properties": {
            "service": {
                "type": "string",
                "description": "The service, as `search` names it.",
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
    pub description: String,
    /// What a call costs the caller, in USD micros; absent when it is free.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub price: Option<i64>,
}
