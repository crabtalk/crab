//! What a source fetched, in its database: `lake_items`, `lake_threads` and
//! `lake_records`.

use alloc::{collections::BTreeSet, string::String, vec::Vec};
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
pub fn record(id: &str, feed: &str, now: u64, values: &impl Serialize) -> Result<usize, CallError> {
    Ok(add_records(feed, now, &[(id, values)])?.1)
}

/// Store each of `records`, an id and its values, as a version of that record
/// in `feed` since `now`, when its values differ from its latest. Of an id
/// listed more than once, the last is stored. Answers how many versions were
/// stored and the bytes written.
pub(crate) fn add_records(
    feed: &str,
    now: u64,
    records: &[(&str, impl Serialize)],
) -> Result<(u64, usize), CallError> {
    let mut seen = BTreeSet::new();
    let mut rows: Vec<(&str, String)> = Vec::with_capacity(records.len());
    for (id, values) in records.iter().rev() {
        if seen.insert(*id) {
            rows.push((id, json::to(values)?));
        }
    }
    rows.reverse();
    written(berm_lang::rows::<(u64,)>(
        "INSERT INTO lake_records (id, feed, since, data)
         SELECT j.value ->> 0, ?1, ?2, j.value ->> 1 FROM json_each(?3) AS j
                LEFT JOIN (SELECT id, data, MAX(seq) FROM lake_records
                            WHERE feed = ?1 AND id IN (SELECT value ->> 0 FROM json_each(?3))
                            GROUP BY id) AS l ON l.id = j.value ->> 0
          WHERE l.data IS NOT j.value ->> 1
          ORDER BY j.key
         RETURNING length(CAST(data AS BLOB))",
        &(feed, now, &rows),
    )?)
}

/// Store each of `items`, an id, when it was posted and the item, as shown by
/// `feed`, in order, those `feed` did not already show. Answers how many were
/// new and the bytes written.
pub(crate) fn add_items(
    feed: &str,
    items: &[(&str, u64, impl Serialize)],
) -> Result<(u64, usize), CallError> {
    let rows = items
        .iter()
        .map(|(id, posted, item)| Ok((*id, *posted, json::to(item)?)))
        .collect::<Result<Vec<_>, CallError>>()?;
    written(berm_lang::rows::<(u64,)>(
        "INSERT OR IGNORE INTO lake_items (id, feed, posted, data)
         SELECT value ->> 0, ?1, value ->> 1, value ->> 2 FROM json_each(?2) ORDER BY key
         RETURNING length(CAST(data AS BLOB))",
        &(feed, &rows),
    )?)
}

/// How many rows an insert returned the bytes of, and their sum.
fn written(rows: Vec<(u64,)>) -> Result<(u64, usize), CallError> {
    let bytes = rows.iter().map(|(bytes,)| *bytes as usize).sum();
    Ok((rows.len() as u64, bytes))
}

/// Whether item `id` is stored as shown by `feed`.
pub fn has(feed: &str, id: &str) -> Result<bool, CallError> {
    let found = berm_lang::rows::<(u8,)>(
        "SELECT 1 FROM lake_items WHERE id = ? AND feed = ?",
        &(id, feed),
    )?;
    Ok(!found.is_empty())
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
    match feed {
        Some(feed) => berm_lang::rows::<(u64, String)>(
            "SELECT seq, data FROM lake_items WHERE feed = ? AND seq > ? ORDER BY seq LIMIT ?",
            &(feed, after, limit),
        )?,
        None => berm_lang::rows::<(u64, String)>(
            "SELECT seq, data FROM lake_items WHERE seq > ? ORDER BY seq LIMIT ?",
            &(after, limit),
        )?,
    }
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
        "SELECT r.seq, r.id, r.feed, r.since, r.data
           FROM (SELECT MIN(seq) AS first, MAX(seq) AS last FROM lake_records
                  WHERE id = ? GROUP BY feed) AS f
                JOIN lake_records AS r ON r.seq = f.last
          ORDER BY f.first",
        &(id,),
    )
}

/// Every version of record `id`, of `feed` when given, oldest first.
pub fn versions_of<T: DeserializeOwned>(
    id: &str,
    feed: Option<&str>,
) -> Result<Vec<Version<T>>, CallError> {
    match feed {
        Some(feed) => versions_where(
            "SELECT seq, id, feed, since, data FROM lake_records
              WHERE id = ? AND feed = ? ORDER BY seq",
            &(id, feed),
        ),
        None => versions_where(
            "SELECT seq, id, feed, since, data FROM lake_records WHERE id = ? ORDER BY seq",
            &(id,),
        ),
    }
}

/// Up to `limit` versions stored after `after`, oldest first, of `feed` when
/// given. Pass the last `seq` as `after` for the next page.
pub fn versions<T: DeserializeOwned>(
    feed: Option<&str>,
    after: u64,
    limit: u64,
) -> Result<Vec<Version<T>>, CallError> {
    match feed {
        Some(feed) => versions_where(
            "SELECT seq, id, feed, since, data FROM lake_records
              WHERE feed = ? AND seq > ? ORDER BY seq LIMIT ?",
            &(feed, after, limit),
        ),
        None => versions_where(
            "SELECT seq, id, feed, since, data FROM lake_records WHERE seq > ? ORDER BY seq LIMIT ?",
            &(after, limit),
        ),
    }
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
