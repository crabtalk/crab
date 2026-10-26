//! Which alerts email an account's owners.

use serde::{Deserialize, Serialize};

/// The alerts an account's owners can turn off. Running out of credit and
/// reaching a share of what the plan includes always email them.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Alerts {
    /// Whether a service starting to fail emails them: its first failed run,
    /// then none until one of its runs is done again.
    pub failures: bool,
}
