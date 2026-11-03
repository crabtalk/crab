use alloc::{string::String, vec::Vec};
use berm_lang::{Result, Schema};

const MAX_ENTRIES: u64 = 10_000;
const MAX_CONTENT: usize = 16 * 1024;

#[derive(Schema)]
pub struct Memory {
    pub namespace: String,
    pub key: String,
    pub content: String,
    pub revision: u64,
    pub updated_at: u64,
}

#[derive(Schema)]
pub struct Memories {
    pub items: Vec<Memory>,
    pub next: Option<String>,
}

impl Memory {
    pub fn put(
        namespace: String,
        key: String,
        content: String,
        expected_revision: Option<u64>,
    ) -> Result<Self> {
        if namespace.trim().is_empty()
            || namespace.len() > 64
            || key.trim().is_empty()
            || key.len() > 128
        {
            return Err("Use a namespace of 1–64 bytes and a key of 1–128 bytes".into());
        }
        if content.is_empty() || content.len() > MAX_CONTENT {
            return Err("Memory content must be 1–16384 bytes".into());
        }
        let existing = Self::get(&namespace, &key)?;
        if existing.is_none() {
            let (count,) =
                berm_lang::rows::<(u64,)>("SELECT COUNT(*) FROM agent_memories", &[0u8; 0])?
                    .pop()
                    .unwrap_or_default();
            if count >= MAX_ENTRIES {
                return Err(
                    "This copy holds 10000 memories; delete entries before adding more".into(),
                );
            }
        }
        if expected_revision.is_some_and(|expected| {
            expected != existing.as_ref().map_or(0, |memory| memory.revision)
        }) {
            return Err("The memory revision changed; read it before updating".into());
        }
        let now = berm_lang::now();
        let (revision,) = berm_lang::rows::<(u64,)>(
            "UPDATE agent_memory_clock SET revision = revision + 1 WHERE id = 1 RETURNING revision",
            &[0u8; 0],
        )?
        .pop()
        .ok_or("The revision clock is missing")?;
        let changed = berm_lang::rows::<(u64,)>(
            "INSERT INTO agent_memories (namespace, key, content, revision, updated_at) VALUES (?1, ?2, ?3, ?6, ?4)
             ON CONFLICT (namespace, key) DO UPDATE SET content = excluded.content, revision = excluded.revision, updated_at = excluded.updated_at
             WHERE ?5 IS NULL OR revision = ?5 RETURNING revision",
            &(&namespace, &key, &content, now, expected_revision, revision),
        )?;
        let (revision,) = changed
            .into_iter()
            .next()
            .ok_or("The memory revision changed; read it before updating")?;
        Ok(Self {
            namespace,
            key,
            content,
            revision,
            updated_at: now,
        })
    }

    pub fn get(namespace: &str, key: &str) -> Result<Option<Self>> {
        Ok(berm_lang::rows::<(String, u64, u64)>("SELECT content, revision, updated_at FROM agent_memories WHERE namespace = ? AND key = ?", &(namespace, key))?
            .pop().map(|(content, revision, updated_at)| Self { namespace: namespace.into(), key: key.into(), content, revision, updated_at }))
    }

    pub fn search(
        namespace: &str,
        query: &str,
        after: Option<&str>,
        limit: Option<u64>,
    ) -> Result<Memories> {
        if query.len() > 256 {
            return Err("Search text exceeds 256 bytes".into());
        }
        // A page can contain ten maximum-size memories within the result buffer.
        let limit = limit.unwrap_or(10).clamp(1, 10);
        let mut found = berm_lang::rows::<(String, String, u64, u64)>(
            "SELECT key, content, revision, updated_at FROM agent_memories WHERE namespace = ? AND key > ? AND instr(lower(content), lower(?)) > 0 ORDER BY key LIMIT ?",
            &(namespace, after.unwrap_or(""), query, limit + 1),
        )?;
        let more = found.len() as u64 > limit;
        found.truncate(limit as usize);
        let items: Vec<_> = found
            .into_iter()
            .map(|(key, content, revision, updated_at)| Self {
                namespace: namespace.into(),
                key,
                content,
                revision,
                updated_at,
            })
            .collect();
        let next = more.then(|| items.last().expect("nonempty page").key.clone());
        Ok(Memories { items, next })
    }

    pub fn delete(namespace: &str, key: &str, expected_revision: Option<u64>) -> Result<bool> {
        let removed = berm_lang::rows::<(u64,)>(
            "DELETE FROM agent_memories WHERE namespace = ? AND key = ? AND (?3 IS NULL OR revision = ?3) RETURNING revision",
            &(namespace, key, expected_revision),
        )?;
        if removed.is_empty() && expected_revision.is_some() && Self::get(namespace, key)?.is_some()
        {
            return Err("The memory revision changed; read it before deleting".into());
        }
        Ok(!removed.is_empty())
    }
}
