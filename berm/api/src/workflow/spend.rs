use serde::{Deserialize, Serialize};
use std::fmt::{Debug, Formatter, Result as FmtResult};

/// Internal authorization carried across trusted host boundaries, never in run responses.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Context {
    pub run: String,
    pub attempt: String,
    pub owner: String,
    pub deadline_at: u64,
    pub authorization: String,
}

impl Debug for Context {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.debug_struct("Context")
            .field("run", &self.run)
            .field("attempt", &self.attempt)
            .field("owner", &self.owner)
            .finish_non_exhaustive()
    }
}

impl Context {
    pub const EXPIRED: &'static str = "E0062";
    pub const DEFERRED: &'static str = "E0063";
}
