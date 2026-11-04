use crate::memory::{Memories, Memory};
use alloc::string::String;
use berm_lang::{Result, tool};

/// Create or update shared memory. Revision zero means create only.
#[tool]
pub fn put(
    namespace: String,
    key: String,
    content: String,
    expected_revision: Option<u64>,
) -> Result<Memory> {
    Memory::put(namespace, key, content, expected_revision)
}

/// Read one memory by namespace and key.
#[tool(query)]
pub fn get(namespace: String, key: String) -> Result<Option<Memory>> {
    Memory::get(&namespace, &key)
}

/// Search memory content by ASCII case-insensitive substring; empty query lists the namespace.
#[tool(query)]
pub fn search(
    namespace: String,
    query: String,
    after: Option<String>,
    limit: Option<u64>,
) -> Result<Memories> {
    Memory::search(&namespace, &query, after.as_deref(), limit)
}

/// Delete memory, optionally refusing a changed revision.
#[tool]
pub fn delete(namespace: String, key: String, expected_revision: Option<u64>) -> Result<bool> {
    Memory::delete(&namespace, &key, expected_revision)
}
