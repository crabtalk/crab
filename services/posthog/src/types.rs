use alloc::{collections::BTreeMap, string::String, vec::Vec};
use berm_lang::{Schema, serde_json::Value};

/// PostHog Cloud region. Private and ingestion hosts are selected together.
#[derive(Debug, Clone, Copy, Schema)]
pub enum Region {
    Us,
    Eu,
}

/// An event to ingest. Properties may contain nested JSON values.
#[derive(Debug, Clone, Default, Schema)]
pub struct Event {
    pub event: String,
    pub distinct_id: String,
    pub properties: Option<BTreeMap<String, Value>>,
    /// ISO 8601 event time; omit for the provider's current time.
    pub timestamp: Option<String>,
    /// Stable event UUID for deduplication when resubmitting.
    pub uuid: Option<String>,
}

/// Inputs for remote flag evaluation; this does not record exposure events.
#[derive(Debug, Clone, Default, Schema)]
pub struct Evaluation {
    pub distinct_id: String,
    pub groups: Option<BTreeMap<String, String>>,
    pub person_properties: Option<BTreeMap<String, Value>>,
    pub group_properties: Option<BTreeMap<String, BTreeMap<String, Value>>>,
    pub evaluation_contexts: Option<Vec<String>>,
}

#[derive(Debug, Clone, Schema)]
pub struct Flag {
    pub key: String,
    pub enabled: bool,
    pub variant: Option<String>,
    pub reason: Option<Value>,
    pub metadata: Option<Value>,
}

/// Partial evaluations retain their errors flag; missing flags are not false.
#[derive(Debug, Clone, Schema)]
pub struct Flags {
    pub flags: BTreeMap<String, Flag>,
    pub errors_while_computing_flags: bool,
    pub request_id: Option<String>,
}
