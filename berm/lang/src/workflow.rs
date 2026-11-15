use crate::{
    Schema,
    schema::{Shape, Variant},
};
use serde::{Serialize, de::DeserializeOwned};

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
pub trait Projection<T: Schema>: Branch {
    const CASE: &'static str;
    fn project(self) -> Option<T>;
}
