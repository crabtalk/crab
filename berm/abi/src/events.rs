use alloc::{collections::BTreeSet, string::String, vec::Vec};
use serde::{Deserialize, Serialize};

/// An immutable event in an authorized, ordered history.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event<T> {
    /// Stable within the event source, including across retries.
    pub id: String,
    /// Resumes strictly after this event in the same filtered history.
    pub cursor: String,
    /// Creation time in epoch milliseconds.
    pub at: u64,
    pub data: T,
}

/// A source called without a cursor returns an empty page at its current head.
/// Existing cursors must remain valid across restarts; expired history is an error.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Page<T> {
    pub events: Vec<Event<T>>,
    /// Resumes after the page. An empty page preserves or advances the checkpoint.
    pub cursor: String,
    pub has_more: bool,
}

impl<T> Page<T> {
    /// Check framing before a consumer advances its stored cursor.
    pub fn check(&self, after: Option<&str>) -> Result<(), &'static str> {
        if self.cursor.is_empty() || self.cursor.len() > 1024 {
            return Err("Invalid event page cursor");
        }
        if after.is_none() && (!self.events.is_empty() || self.has_more) {
            return Err("An initial event page must be an empty checkpoint");
        }
        if self.has_more && self.events.is_empty() {
            return Err("An event page with more history must make progress");
        }
        let mut ids = BTreeSet::new();
        let mut cursors = BTreeSet::new();
        for event in &self.events {
            if event.id.is_empty() || event.id.len() > 256 || !ids.insert(&event.id) {
                return Err("Invalid or repeated event id");
            }
            if event.cursor.is_empty()
                || event.cursor.len() > 1024
                || Some(event.cursor.as_str()) == after
                || !cursors.insert(&event.cursor)
            {
                return Err("Invalid or repeated event cursor");
            }
        }
        if self
            .events
            .last()
            .is_some_and(|event| event.cursor != self.cursor)
        {
            return Err("Event page cursor must identify its last event");
        }
        Ok(())
    }
}
