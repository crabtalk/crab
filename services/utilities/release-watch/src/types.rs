use alloc::{string::String, vec::Vec};
use berm_lang::{Schema, serde_json::Value};
use github::types::Release;
use osv::types::PackageVersion;

/// A named installed package and its explicitly mapped GitHub repository.
#[derive(Debug, Clone, PartialEq, Eq, Schema)]
pub struct Watch {
    pub name: String,
    pub package: PackageVersion,
    pub owner: String,
    pub repo: String,
}

/// A complete OSV match set and one recent GitHub release page, not a version comparison.
#[derive(Debug, Clone, Schema)]
pub struct Snapshot {
    pub releases: Vec<Release>,
    /// More release pages exist. Only the most recent 30 records were observed.
    pub releases_next_page: Option<u64>,
    pub advisories: Vec<Value>,
}

/// Watch state. A failed check keeps the last successful snapshot intact.
#[derive(Debug, Clone, Schema)]
pub struct Status {
    pub watch: Watch,
    pub attempted_at: Option<u64>,
    pub succeeded_at: Option<u64>,
    pub error: Option<String>,
    pub snapshot: Option<Snapshot>,
}

/// Retained observation. Kind is baseline, changed, failed or recovered.
#[derive(Debug, Clone, Schema)]
pub struct Change {
    pub cursor: u64,
    pub observed_at: u64,
    pub watch: Watch,
    pub kind: String,
    pub snapshot: Option<Snapshot>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Schema)]
pub struct Changes {
    pub items: Vec<Change>,
    /// Pass as after on the next read. Reading never acknowledges or deletes records.
    pub next_cursor: u64,
    pub has_more: bool,
}

#[derive(Debug, Clone, Default, Schema)]
pub struct Refreshed {
    pub checked: u64,
    pub failed: u64,
}
