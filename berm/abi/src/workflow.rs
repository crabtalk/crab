use alloc::{collections::BTreeMap, string::String, vec::Vec};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlInput {
    pub input: Value,
    pub nodes: BTreeMap<String, ControlValue>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum ControlValue {
    Present { value: Value },
    Skipped,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ControlOutput {
    Calls { inputs: Vec<Value> },
    Skipped,
    Branch { case: String, value: Value },
    Output { value: Value },
}

/// The workflow run an invocation is a step of, as the host answers
/// `berm.workflow.run`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Run {
    pub run: String,
    pub workflow: String,
    pub attempt: String,
    /// The step's node ID.
    pub node: String,
    /// The item's index in an `each` step. Zero for a single-call step.
    pub item: u32,
    pub trigger: Trigger,
}

/// What started a run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Trigger {
    /// A caller started it: the account, and the API key and verified end user
    /// it came with.
    Manual {
        account: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        key: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        user: Option<User>,
    },
    /// The deployment's schedule started it, for the occurrence at `at`,
    /// milliseconds since the Unix epoch.
    Schedule { at: u64 },
}

/// An end user as the host verified them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct User {
    /// The identity provider or service that vouched for the user.
    pub issuer: String,
    /// Who they are to that issuer.
    pub subject: String,
}

#[derive(Clone, Copy)]
pub struct Node<'a> {
    pub kind: Kind,
    pub dependencies: &'a [Dependency<'a>],
}

#[derive(Clone, Copy)]
pub enum Kind {
    Step { maximum: u32, optional: bool },
    Branch,
    Output,
}

#[derive(Clone, Copy)]
pub struct Dependency<'a> {
    pub producer: Option<usize>,
    pub optional: bool,
    pub case: Option<&'a str>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Selection<'a> {
    Present,
    Case(&'a str),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Invalid {
    pub node: usize,
    pub kind: InvalidKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InvalidKind {
    Capacity,
    Reference,
    Each,
    Join,
    Output,
    Disconnected,
    Input,
}

/// `guards` has N² slots and `reachable` N slots; both are scratch space.
pub const fn validate<'a>(
    nodes: &[Node<'a>],
    guards: &mut [Option<Selection<'a>>],
    reachable: &mut [bool],
    max_each: u32,
) -> Result<(), Invalid> {
    let n = nodes.len();
    let Some(capacity) = n.checked_mul(n) else {
        return Err(Invalid {
            node: 0,
            kind: InvalidKind::Capacity,
        });
    };
    if guards.len() < capacity || reachable.len() < n {
        return Err(Invalid {
            node: 0,
            kind: InvalidKind::Capacity,
        });
    }
    let mut at = 0;
    while at < capacity {
        guards[at] = None;
        at += 1;
    }
    at = 0;
    while at < n {
        reachable[at] = false;
        at += 1;
    }
    let mut output = None;
    let mut input = false;
    at = 0;
    while at < n {
        if let Kind::Step { maximum, .. } = nodes[at].kind
            && (maximum == 0 || maximum > max_each)
        {
            return Err(Invalid {
                node: at,
                kind: InvalidKind::Each,
            });
        }
        let mut dep = 0;
        while dep < nodes[at].dependencies.len() {
            let dependency = nodes[at].dependencies[dep];
            match dependency.producer {
                None => {
                    input = true;
                    if dependency.case.is_some() {
                        return Err(Invalid {
                            node: at,
                            kind: InvalidKind::Reference,
                        });
                    }
                }
                Some(producer) => {
                    if producer >= at || matches!(nodes[producer].kind, Kind::Output) {
                        return Err(Invalid {
                            node: at,
                            kind: InvalidKind::Reference,
                        });
                    }
                    if !dependency.optional {
                        let mut branch = 0;
                        while branch < n {
                            let inherited = guards[producer * n + branch];
                            let selected = if branch == producer {
                                match dependency.case {
                                    Some(case) => Some(Selection::Case(case)),
                                    None if matches!(
                                        nodes[producer].kind,
                                        Kind::Step { optional: true, .. }
                                    ) =>
                                    {
                                        Some(Selection::Present)
                                    }
                                    None => inherited,
                                }
                            } else {
                                inherited
                            };
                            if let Some(selected) = selected {
                                let existing = guards[at * n + branch];
                                match (existing, selected) {
                                    (Some(Selection::Case(a)), Selection::Case(b)) => {
                                        if !equal(a, b) {
                                            return Err(Invalid {
                                                node: at,
                                                kind: InvalidKind::Join,
                                            });
                                        }
                                    }
                                    (Some(Selection::Case(_)), Selection::Present) => {}
                                    _ => guards[at * n + branch] = Some(selected),
                                }
                            }
                            branch += 1;
                        }
                    }
                }
            }
            dep += 1;
        }
        if matches!(nodes[at].kind, Kind::Output) {
            if output.is_some() {
                return Err(Invalid {
                    node: at,
                    kind: InvalidKind::Output,
                });
            }
            let mut branch = 0;
            while branch < n {
                if guards[at * n + branch].is_some() {
                    return Err(Invalid {
                        node: at,
                        kind: InvalidKind::Output,
                    });
                }
                branch += 1;
            }
            output = Some(at);
        }
        at += 1;
    }
    let Some(output) = output else {
        return Err(Invalid {
            node: 0,
            kind: InvalidKind::Output,
        });
    };
    if !input {
        return Err(Invalid {
            node: 0,
            kind: InvalidKind::Input,
        });
    }
    reachable[output] = true;
    at = n;
    while at > 0 {
        at -= 1;
        if reachable[at] {
            let mut dep = 0;
            while dep < nodes[at].dependencies.len() {
                if let Some(producer) = nodes[at].dependencies[dep].producer {
                    reachable[producer] = true;
                }
                dep += 1;
            }
        }
    }
    at = 0;
    while at < n {
        if !reachable[at] {
            return Err(Invalid {
                node: at,
                kind: InvalidKind::Disconnected,
            });
        }
        at += 1;
    }
    Ok(())
}

const fn equal(a: &str, b: &str) -> bool {
    let a = a.as_bytes();
    let b = b.as_bytes();
    if a.len() != b.len() {
        return false;
    }
    let mut at = 0;
    while at < a.len() {
        if a[at] != b[at] {
            return false;
        }
        at += 1;
    }
    true
}

impl InvalidKind {
    pub const fn message(self) -> &'static str {
        match self {
            Self::Capacity => "workflow validation scratch capacity is too small",
            Self::Reference => "dependency must name an earlier producer",
            Self::Each => "each bound must be positive and within the platform cap",
            Self::Join => {
                "required dependencies select mutually exclusive paths; use optional references"
            }
            Self::Output => {
                "exactly one output must be reachable on every path; use optional references for guarded values"
            }
            Self::Disconnected => "node does not contribute to the workflow output",
            Self::Input => "workflow needs an input dependency",
        }
    }
}
