use crate::{
    compat, digest,
    workflow::{Version, run::Binding},
};
use anyhow::{Result, bail, ensure};
use berm_abi::workflow as validation;
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
        Ok(digest::prefixed(&digest::json_hex(&serde_json::to_value(
            self,
        )?)))
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
    pub producer: String,
    pub selection: Selection,
}

#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Selection {
    Present,
    Case { name: String },
}

impl Graph {
    pub fn check(&self, max_each: u32) -> Result<()> {
        let mut dependencies = Vec::new();
        for (index, node) in self.nodes.iter().enumerate() {
            ensure!(!node.id.is_empty(), "workflow node has an empty ID");
            ensure!(
                !self.nodes[..index]
                    .iter()
                    .any(|other| other.id == node.id || other.export == node.export),
                "duplicate workflow node or export {}",
                node.id
            );
            let mut inputs = Vec::new();
            for dependency in &node.dependencies {
                let producer = match &dependency.source {
                    Producer::Input => None,
                    Producer::Node { id } => Some(
                        self.nodes[..index]
                            .iter()
                            .position(|node| node.id == *id)
                            .ok_or_else(|| {
                                anyhow::anyhow!("{} depends on unknown or later node {id}", node.id)
                            })?,
                    ),
                };
                if let Some(case) = &dependency.case {
                    let source = producer
                        .map(|index| &self.nodes[index])
                        .ok_or_else(|| anyhow::anyhow!("workflow input cannot select a case"))?;
                    let cases = match &source.kind {
                        Kind::Step {
                            call,
                            mode: Mode::Once | Mode::Optional,
                        } => &call.cases,
                        Kind::Branch { cases } => cases,
                        _ => bail!("{} selects a case of a producer without cases", node.id),
                    };
                    ensure!(
                        cases.iter().any(|candidate| candidate.name == *case),
                        "{} selects unknown case {case}",
                        node.id
                    );
                }
                inputs.push(validation::Dependency {
                    producer,
                    optional: dependency.optional,
                    case: dependency.case.as_deref(),
                });
            }
            dependencies.push(inputs);
        }
        let nodes: Vec<_> = self
            .nodes
            .iter()
            .zip(&dependencies)
            .map(|(node, dependencies)| validation::Node {
                kind: match &node.kind {
                    Kind::Step { mode, .. } => validation::Kind::Step {
                        maximum: mode.maximum(),
                        optional: *mode == Mode::Optional,
                    },
                    Kind::Branch { .. } => validation::Kind::Branch,
                    Kind::Output => validation::Kind::Output,
                },
                dependencies,
            })
            .collect();
        let mut guards = vec![None; nodes.len() * nodes.len()];
        let mut reachable = vec![false; nodes.len()];
        if let Err(error) = validation::validate(&nodes, &mut guards, &mut reachable, max_each) {
            let name = self
                .nodes
                .get(error.node)
                .map_or("workflow", |node| node.id.as_str());
            bail!("{name}: {}", error.kind.message());
        }
        for (index, node) in self.nodes.iter().enumerate() {
            let expected: Vec<_> = self
                .nodes
                .iter()
                .enumerate()
                .filter_map(|(producer, source)| {
                    let selection = match guards[index * nodes.len() + producer]? {
                        validation::Selection::Present => Selection::Present,
                        validation::Selection::Case(name) => Selection::Case { name: name.into() },
                    };
                    Some(Guard {
                        producer: source.id.clone(),
                        selection,
                    })
                })
                .collect();
            ensure!(
                node.guards == expected,
                "{} has guards inconsistent with its dependencies",
                node.id
            );
        }
        Ok(())
    }
}

impl Target {
    pub fn tool(&self) -> &str {
        match self {
            Self::Local { tool } | Self::Service { tool, .. } => tool,
        }
    }
}

impl Call {
    pub fn check_binding(&self, binding: &Binding) -> Result<()> {
        ensure!(
            self.target == binding.target,
            "workflow binding targets another tool"
        );
        if let Target::Service {
            idempotency_ms: Some(requested),
            ..
        } = self.target
        {
            ensure!(
                requested > 0
                    && binding
                        .idempotency_ms
                        .is_some_and(|available| available >= requested),
                "target does not provide the declared idempotency window"
            );
        }
        self.check_schemas(&binding.input, Some(&binding.output))
    }

    pub fn check_schemas(&self, input: &Value, output: Option<&Value>) -> Result<()> {
        let mut breaks = Vec::new();
        compat::accepts("input", &self.input, input, &mut breaks);
        if let Some(output) = output {
            compat::writes("output", &self.output, output, &mut breaks);
        } else if self.output != serde_json::json!({"type":"null"}) {
            breaks.push("tool has no declared output".into());
        }
        ensure!(
            breaks.is_empty(),
            "incompatible workflow dependency: {}",
            breaks.join("; ")
        );
        Ok(())
    }
}
