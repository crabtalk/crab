//! What a source fetched, in its database: `lake_items` and `lake_threads`.

use alloc::{string::String, vec::Vec};
use berm_lang::{
    CallError, json,
    serde::{Serialize, de::DeserializeOwned},
};

/// An item as stored: its position among every item stored, and the item.
pub struct Stored<T> {
    pub seq: u64,
    pub item: T,
}

/// Store `item`, posted at `posted`, as shown by `feed`. Answers the bytes
/// written, 0 when `feed` already showed it.
pub(crate) fn item(
    id: &str,
    feed: &str,
    posted: u64,
    item: &impl Serialize,
) -> Result<usize, CallError> {
    let data = json::to(item)?;
    let inserted = berm_lang::rows::<(u64,)>(
        "INSERT OR IGNORE INTO lake_items (id, feed, posted, data) VALUES (?, ?, ?, ?)
         RETURNING seq",
        &(id, feed, posted, &data),
    )?;
    Ok(if inserted.is_empty() { 0 } else { data.len() })
}

/// Store `thread` as the last fetched under item `id`, at `now`. Answers the
/// bytes written.
pub(crate) fn thread(id: &str, now: u64, thread: &impl Serialize) -> Result<usize, CallError> {
    let data = json::to(thread)?;
    berm_lang::rows::<()>(
        "INSERT INTO lake_threads (id, data, fetched) VALUES (?, ?, ?)
         ON CONFLICT (id) DO UPDATE SET data = excluded.data, fetched = excluded.fetched",
        &(id, &data, now),
    )?;
    Ok(data.len())
}

/// Up to `limit` items stored after `after`, oldest first, of `feed` when
/// given.
pub fn items<T: DeserializeOwned>(
    feed: Option<&str>,
    after: u64,
    limit: u64,
) -> Result<Vec<Stored<T>>, CallError> {
    berm_lang::rows::<(u64, String)>(
        "SELECT seq, data FROM lake_items WHERE seq > ? AND (? IS NULL OR feed = ?)
          ORDER BY seq LIMIT ?",
        &(after, feed, feed, limit),
    )?
    .into_iter()
    .map(|(seq, data)| {
        Ok(Stored {
            seq,
            item: json::from(&data)?,
        })
    })
    .collect()
}

/// The last thread fetched under item `id`.
pub fn thread_of<T: DeserializeOwned>(id: &str) -> Result<Option<T>, CallError> {
    berm_lang::rows::<(String,)>("SELECT data FROM lake_threads WHERE id = ?", &(id,))?
        .first()
        .map(|(data,)| json::from(data))
        .transpose()
}
