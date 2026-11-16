use crate::memory::{Memories, Memory};
use alloc::string::String;
use berm_lang::{Result, tool};

/// Save memory
///
/// Create or update shared memory. Revision zero means create only.
#[tool(
    catalog = "memory.put",
    budget(cpu = 100000, read = 20000, written = 10, memory = 16)
)]
pub fn put(
    namespace: String,
    key: String,
    content: String,
    expected_revision: Option<u64>,
) -> Result<Memory> {
    Memory::put(namespace, key, content, expected_revision)
}

/// Read memory
///
/// Read one memory by namespace and key.
#[tool(
    query,
    catalog = "memory.get",
    budget(cpu = 100000, read = 20000, written = 10, memory = 16)
)]
pub fn get(namespace: String, key: String) -> Result<Option<Memory>> {
    Memory::get(&namespace, &key)
}

/// Search memories
///
/// Search memory content by ASCII case-insensitive substring; empty query lists the namespace.
#[tool(
    query,
    catalog = "memory.search",
    budget(cpu = 100000, read = 20000, written = 10, memory = 16)
)]
pub fn search(
    namespace: String,
    query: String,
    after: Option<String>,
    limit: Option<u64>,
) -> Result<Memories> {
    Memory::search(&namespace, &query, after.as_deref(), limit)
}

/// Delete memory
///
/// Delete memory, optionally refusing a changed revision.
#[tool(
    catalog = "memory.delete",
    budget(cpu = 100000, read = 20000, written = 10, memory = 16)
)]
pub fn delete(namespace: String, key: String, expected_revision: Option<u64>) -> Result<bool> {
    Memory::delete(&namespace, &key, expected_revision)
}
