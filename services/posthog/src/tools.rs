use crate::{Evaluation, Event, Flags, PostHog, Region};
use alloc::{string::String, vec::Vec};
use berm_lang::{Result, serde_json::Value, tool};
fn client() -> Result<PostHog> {
    let region = match berm_lang::var("POSTHOG_REGION")?.as_str() {
        "us" => Region::Us,
        "eu" => Region::Eu,
        _ => return Err("POSTHOG_REGION must be us or eu".into()),
    };
    Ok(PostHog::new(region))
}

/// Submit an event to PostHog. Acceptance does not mean it is immediately queryable.
#[tool]
pub fn capture(
    /// Named event, distinct user ID and optional properties.
    event: Event,
) -> Result<()> {
    client()?.capture(&event)
}

/// Submit events in one request. Stable event UUIDs help deduplicate retries.
#[tool]
pub fn capture_batch(
    /// Events to submit.
    events: Vec<Event>,
) -> Result<()> {
    client()?.capture_batch(&events)
}

/// Evaluate feature flags for a user. Inspect partial errors; this does not log exposure.
#[tool]
pub fn flags(
    /// User ID and optional group, person and evaluation context properties.
    evaluation: Evaluation,
) -> Result<Flags> {
    client()?.flags(&evaluation)
}

/// Run a named HogQL analytics query using the deployment's private API key.
#[tool]
pub fn query(
    /// HogQL SQL, with an appropriate LIMIT for row results.
    sql: String,
    /// Descriptive query name for the provider's query log.
    name: String,
) -> Result<Value> {
    let project_id = berm_lang::var("POSTHOG_PROJECT_ID")?
        .parse()
        .map_err(|_| "Set POSTHOG_PROJECT_ID to your numeric project ID")?;
    client()?.query(project_id, &sql, &name)
}
