use crate::{BatchPage, PackageVersion, Page, Query};
use alloc::{string::String, vec::Vec};
use berm_lang::{Result, serde_json::Value, tool};

/// Read one page of OSV advisories matching a package's installed version.
#[tool]
pub fn query(
    /// Exact ecosystem, package name and installed version.
    package: PackageVersion,
    /// Token from the preceding page; omit on the first request.
    page_token: Option<String>,
) -> Result<Page> {
    crate::query(&package, page_token.as_deref())
}

/// Query multiple packages. Each result has its own optional pagination token.
#[tool]
pub fn query_batch(
    /// Up to 1000 package queries, in result order.
    queries: Vec<Query>,
) -> Result<Vec<BatchPage>> {
    crate::query_batch(&queries)
}

/// Read a full advisory, including affected ranges, references and provider metadata.
#[tool]
pub fn advisory(
    /// Case-sensitive OSV advisory identifier.
    id: String,
) -> Result<Value> {
    crate::advisory(&id)
}
