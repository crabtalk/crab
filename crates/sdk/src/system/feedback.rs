//! Client feedback wire types.

use serde::{Deserialize, Serialize};

#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedbackRequest {
    /// What the person wrote.
    pub body: String,
    /// The client build it was written from.
    pub version: Option<String>,
}
