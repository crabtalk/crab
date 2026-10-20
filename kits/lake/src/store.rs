//! What a source fetched, in its database: `lake_items`, `lake_threads` and
//! `lake_records`.

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

/// A version of a record: its position among every version stored, the
/// record's id and feed, when the version began, and its values.
pub struct Version<T> {
    pub seq: u64,
    pub id: String,
    pub feed: String,
    /// When it was stored, in Unix milliseconds.
    pub since: u64,
    pub values: T,
}

/// Store `values` as a version of record `id` in `feed` since `now`, when they
/// differ from its latest. Answers the bytes written, 0 when they do not.
pub(crate) fn record(
    id: &str,
    feed: &str,
    now: u64,
    values: &impl Serialize,
) -> Result<usize, CallError> {
    let data = json::to(values)?;
    let latest = berm_lang::rows::<(String,)>(
        "SELECT data FROM lake_records WHERE id = ? AND feed = ? ORDER BY seq DESC LIMIT 1",
        &(id, feed),
    )?;
    if latest.first().is_some_and(|(stored,)| *stored == data) {
        return Ok(0);
    }
    berm_lang::rows::<()>(
        "INSERT INTO lake_records (id, feed, since, data) VALUES (?, ?, ?, ?)",
        &(id, feed, now, &data),
    )?;
    Ok(data.len())
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

/// Whether item `id` is stored as shown by `feed`.
pub fn has(feed: &str, id: &str) -> Result<bool, CallError> {
    let found = berm_lang::rows::<(u8,)>(
        "SELECT 1 FROM lake_items WHERE id = ? AND feed = ?",
        &(id, feed),
    )?;
    Ok(!found.is_empty())
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

/// The latest version of record `id` in each feed that showed it, in the
/// order the feeds first did.
pub fn record_of<T: DeserializeOwned>(id: &str) -> Result<Vec<Version<T>>, CallError> {
    versions_where(
        "SELECT seq, id, feed, since, data FROM lake_records AS r
          WHERE id = ? AND seq = (SELECT MAX(seq) FROM lake_records
                                   WHERE id = r.id AND feed = r.feed)
          ORDER BY (SELECT MIN(seq) FROM lake_records WHERE id = r.id AND feed = r.feed)",
        &(id,),
    )
}

/// Every version of record `id`, of `feed` when given, oldest first.
pub fn versions_of<T: DeserializeOwned>(
    id: &str,
    feed: Option<&str>,
) -> Result<Vec<Version<T>>, CallError> {
    versions_where(
        "SELECT seq, id, feed, since, data FROM lake_records
          WHERE id = ? AND (? IS NULL OR feed = ?) ORDER BY seq",
        &(id, feed, feed),
    )
}

/// Up to `limit` versions stored after `after`, oldest first, of `feed` when
/// given. Pass the last `seq` as `after` for the next page.
pub fn versions<T: DeserializeOwned>(
    feed: Option<&str>,
    after: u64,
    limit: u64,
) -> Result<Vec<Version<T>>, CallError> {
    versions_where(
        "SELECT seq, id, feed, since, data FROM lake_records
          WHERE seq > ? AND (? IS NULL OR feed = ?) ORDER BY seq LIMIT ?",
        &(after, feed, feed, limit),
    )
}

fn versions_where<T: DeserializeOwned>(
    statement: &str,
    params: &impl Serialize,
) -> Result<Vec<Version<T>>, CallError> {
    berm_lang::rows::<(u64, String, String, u64, String)>(statement, params)?
        .into_iter()
        .map(|(seq, id, feed, since, data)| {
            Ok(Version {
                seq,
                id,
                feed,
                since,
                values: json::from(&data)?,
            })
        })
        .collect()
}
