use crate::{
    CallError, Schema, host,
    schema::{Shape, Variant},
    sys,
};
use serde::{Serialize, de::DeserializeOwned};

pub use crate::abi::workflow::{Run, Trigger};

/// The run that dispatched this tool. Returns `None` for ordinary calls and
/// calls made by the step's tool. Workflow controls cannot read it.
pub fn run() -> Result<Option<Run>, CallError> {
    let reply = host::call(sys::WORKFLOW_RUN, &[])?;
    if reply.is_empty() {
        return Ok(None);
    }
    serde_json::from_slice(&reply)
        .map(Some)
        .map_err(|error| CallError::Failed(alloc::format!("invalid workflow context: {error}")))
}

pub trait Call: Schema + Serialize + DeserializeOwned {
    const TARGET: Target;
    type Output: Schema + Serialize + DeserializeOwned;
    const CASES: &'static [Variant] = match Self::Output::SHAPE {
        Shape::Union { variants, .. } => variants,
        _ => &[],
    };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    Local {
        tool: &'static str,
    },
    Service {
        service: &'static str,
        tool: &'static str,
        idempotency_ms: Option<u64>,
    },
}

pub trait Branch: Schema + Serialize + DeserializeOwned {
    const CASES: &'static [Variant];
    fn case(&self) -> &'static str;
}

/// A selected payload is moved into the control adapter, which borrows it for the builder.
pub trait Projection<T: Schema>: Schema + Serialize + DeserializeOwned {
    const CASE: Option<&'static str>;
    fn project(self) -> Option<T>;
}

impl<T: Schema + Serialize + DeserializeOwned> Projection<T> for T {
    const CASE: Option<&'static str> = None;
    fn project(self) -> Option<T> {
        Some(self)
    }
}
