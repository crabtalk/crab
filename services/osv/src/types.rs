use alloc::{string::String, vec::Vec};
use berm_lang::{Schema, serde_json::Value};

/// An exact package ecosystem, name and installed version. No repository inference.
#[derive(Debug, Clone, PartialEq, Eq, Schema)]
pub struct PackageVersion {
    pub ecosystem: String,
    pub name: String,
    pub version: String,
}

/// One page of complete OSV advisory records, including provider-specific fields.
#[derive(Debug, Clone, Schema)]
pub struct Page {
    pub vulns: Vec<Value>,
    pub next_page_token: Option<String>,
}

/// One batch input; repeat only paginated inputs with their respective tokens.
#[derive(Debug, Clone, Schema)]
pub struct Query {
    pub package: PackageVersion,
    pub page_token: Option<String>,
}

#[derive(Debug, Clone, Schema)]
pub struct AdvisoryRef {
    pub id: String,
    pub modified: String,
}

/// Batch results contain identifiers and modification times, not full records.
#[derive(Debug, Clone, Schema)]
pub struct BatchPage {
    pub vulns: Vec<AdvisoryRef>,
    pub next_page_token: Option<String>,
}
