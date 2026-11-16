use crate::Meters;
use berm_abi::workflow::ControlInput;
use std::{fmt, sync::Arc, time::Instant};

pub struct Request<'a> {
    pub workflow: &'a str,
    pub node: &'a str,
    pub input: &'a ControlInput,
    pub deadline: Instant,
    pub meters: Arc<Meters>,
}

#[derive(Debug)]
pub struct Effect {
    pub syscall: String,
}

impl Effect {
    pub fn explanation(error: &anyhow::Error) -> Option<&'static str> {
        error
            .chain()
            .any(|cause| cause.downcast_ref::<Self>().is_some())
            .then_some(Self::EXPLAIN)
    }

    pub const EXPLAIN: &'static str = "E0059";
}

impl fmt::Display for Effect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} is not permitted in workflow controls", self.syscall)
    }
}
impl std::error::Error for Effect {}
