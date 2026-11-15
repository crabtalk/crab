use crate::{digest, workflow::Version};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    pub version: Version,
    pub name: String,
    pub graph: Graph,
}

#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Graph {
    pub input: Value,
    pub output: Value,
    /// Declaration order; dependencies name earlier nodes.
    pub nodes: Vec<Node>,
}

impl Entry {
    /// SHA-256 of compact JSON with recursively sorted object keys. Array order is significant.
    pub fn digest(&self) -> Result<String, serde_json::Error> {
        let mut value = serde_json::to_value(self)?;
        value.sort_all_objects();
        Ok(digest::of(&serde_json::to_vec(&value)?))
    }
}

#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Source {
    pub image: String,
    pub graph: String,
}

#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Node {
    pub id: String,
    pub export: String,
    pub dependencies: Vec<Dependency>,
    /// Conjunction of required branch selections.
    pub guards: Vec<Guard>,
    #[serde(flatten)]
    pub kind: Kind,
}

#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Kind {
    Step { call: Call, mode: Mode },
    Branch { cases: Vec<Case> },
    Output,
}

#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Mode {
    Once,
    Optional,
    Each { max: u32 },
}

impl Mode {
    pub fn maximum(self) -> u32 {
        match self {
            Self::Once | Self::Optional => 1,
            Self::Each { max } => max,
        }
    }
}

#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Call {
    pub target: Target,
    pub input: Value,
    pub output: Value,
    pub cases: Vec<Case>,
}

#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Target {
    Local {
        tool: String,
    },
    Service {
        service: String,
        tool: String,
        /// None forbids redispatch after an unknown outcome.
        idempotency_ms: Option<u64>,
    },
}

#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Case {
    pub name: String,
    pub schema: Value,
}

#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Dependency {
    pub source: Producer,
    /// Only a skipped producer supplies absence. Pending, failed and unknown never do.
    pub optional: bool,
    pub case: Option<String>,
}

#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Producer {
    Input,
    Node { id: String },
}

#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Guard {
    pub branch: String,
    pub case: String,
}
