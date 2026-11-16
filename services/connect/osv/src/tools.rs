use crate::{
    api,
    types::{BatchPage, PackageVersion, Page, Query},
};
use alloc::{string::String, vec::Vec};
use berm_lang::{Result, serde_json::Value, tool};

/// Query vulnerabilities
///
/// Read one page of OSV advisories matching a package's installed version.
#[tool(
    catalog = "packages.vulnerabilities",
    budget(cpu = 100000, memory = 64)
)]
pub fn query(
    /// Exact ecosystem, package name and installed version.
    package: PackageVersion,
    /// Token from the preceding page; omit on the first request.
    page_token: Option<String>,
) -> Result<Page> {
    api::query(&package, page_token.as_deref())
}

/// Query batch
///
/// Query multiple packages. Each result has its own optional pagination token.
#[tool(
    catalog = "packages.vulnerabilities.batch",
    budget(cpu = 100000, memory = 64)
)]
pub fn query_batch(
    /// Up to 1000 package queries, in result order.
    queries: Vec<Query>,
) -> Result<Vec<BatchPage>> {
    api::query_batch(&queries)
}

/// Read advisory
///
/// Read a full advisory, including affected ranges, references and provider metadata.
#[tool(catalog = "packages.advisory", budget(cpu = 100000, memory = 64))]
pub fn advisory(
    /// Case-sensitive OSV advisory identifier.
    id: String,
) -> Result<Value> {
    api::advisory(&id)
}
