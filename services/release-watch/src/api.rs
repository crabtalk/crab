use crate::{Change, Changes, Refreshed, Snapshot, Status, Watch};
use alloc::{format, string::String, vec::Vec};
use berm_lang::{
    Result, rows,
    serde::{Serialize, de::DeserializeOwned},
    serde_json,
};

const HOUR: u64 = 3_600_000;
fn encode(value: &impl Serialize) -> Result<String> {
    serde_json::to_string(value).map_err(|e| format!("Release watch JSON: {e}").into())
}
fn decode<T: DeserializeOwned>(value: &str) -> Result<T> {
    serde_json::from_str(value).map_err(|_| "Release watch stored unreadable data".into())
}
fn check_name(name: &str) -> Result<()> {
    if name.trim().is_empty() || name.len() > 128 {
        return Err("Watch names must contain 1 to 128 bytes".into());
    }
    Ok(())
}
/// Add or update a watch. Changing its mapping starts a new baseline, preserving history.
pub fn follow(watch: &Watch) -> Result<()> {
    check_name(&watch.name)?;
    if [
        &watch.package.ecosystem,
        &watch.package.name,
        &watch.package.version,
        &watch.owner,
        &watch.repo,
    ]
    .iter()
    .any(|v| v.trim().is_empty() || v.len() > 512)
    {
        return Err("Provide package ecosystem, name, version and explicit repository owner and name (at most 512 bytes each)".into());
    }
    let spec = encode(watch)?;
    let existing = rows::<(String,)>(
        "SELECT spec FROM release_watch_watches WHERE name = ?",
        &(&watch.name,),
    )?;
    if existing
        .first()
        .is_some_and(|(previous,)| previous == &spec)
    {
        return Ok(());
    }
    if existing.is_empty() {
        let count = rows::<(u64,)>("SELECT count(*) FROM release_watch_watches", &[0_u8; 0])?[0].0;
        if count >= 20 {
            return Err("A deployment supports at most 20 watched packages".into());
        }
    }
    rows::<()>(
        "INSERT INTO release_watch_watches(name,spec) VALUES (?,?) ON CONFLICT(name) DO UPDATE SET spec=excluded.spec, attempted_at=NULL, succeeded_at=NULL, error=NULL, snapshot=NULL",
        &(&watch.name, &spec),
    )?;
    Ok(())
}
/// Stop checking a watch. Its retained changes remain readable.
pub fn unfollow(name: &str) -> Result<()> {
    rows::<()>("DELETE FROM release_watch_watches WHERE name = ?", &(name,))?;
    Ok(())
}
type Stored = (
    String,
    Option<u64>,
    Option<u64>,
    Option<String>,
    Option<String>,
);
fn status((spec, attempted_at, succeeded_at, error, snapshot): Stored) -> Result<Status> {
    Ok(Status {
        watch: decode(&spec)?,
        attempted_at,
        succeeded_at,
        error,
        snapshot: snapshot.map(|v| decode(&v)).transpose()?,
    })
}
/// Read every active watch and its last successful data and current failure status.
pub fn watches() -> Result<Vec<Status>> {
    let stored = rows::<Stored>(
        "SELECT spec,attempted_at,succeeded_at,error,snapshot FROM release_watch_watches ORDER BY name",
        &[0_u8; 0],
    )?;
    stored.into_iter().map(status).collect()
}
/// Read cursor-paginated history, oldest first. Data remains until the deployment removes it.
pub fn changes(after: u64, limit: Option<u64>) -> Result<Changes> {
    let limit = limit.unwrap_or(20).clamp(1, 100);
    let stored = rows::<(u64, u64, String, String, Option<String>, Option<String>)>(
        "SELECT id,observed_at,spec,kind,snapshot,error FROM release_watch_changes WHERE id > ? ORDER BY id LIMIT ?",
        &(after, limit + 1),
    )?;
    let mut items = stored
        .into_iter()
        .map(|(cursor, observed_at, spec, kind, snapshot, error)| {
            Ok(Change {
                cursor,
                observed_at,
                watch: decode(&spec)?,
                kind,
                snapshot: snapshot.map(|v| decode(&v)).transpose()?,
                error,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    let has_more = items.len() > limit as usize;
    items.truncate(limit as usize);
    Ok(Changes {
        next_cursor: items.last().map_or(after, |item| item.cursor),
        items,
        has_more,
    })
}
/// Fetch provider evidence. Release tags are not compared to package versions or claimed as fixes.
pub fn observe(watch: &Watch) -> Result<Snapshot> {
    let mut releases = github::releases(
        &watch.owner,
        &watch.repo,
        &github::Page {
            page: Some(1),
            per_page: Some(30),
        },
    )?;
    releases.items.sort_by_key(|release| release.id);
    let advisories = osv::query_all(&watch.package)?;
    Ok(Snapshot {
        releases: releases.items,
        releases_next_page: releases.next_page,
        advisories,
    })
}
fn record(
    previous: &Status,
    now: u64,
    kind: &str,
    snapshot: Option<&str>,
    error: Option<&str>,
) -> Result<()> {
    rows::<()>(
        "INSERT INTO release_watch_changes(observed_at,spec,kind,snapshot,error) VALUES (?,?,?,?,?)",
        &(now, encode(&previous.watch)?, kind, snapshot, error),
    )?;
    Ok(())
}
/// Check one watch immediately. Provider failures are recorded and returned in status.
pub fn refresh(name: &str) -> Result<Status> {
    let found = rows::<Stored>("SELECT spec,attempted_at,succeeded_at,error,snapshot FROM release_watch_watches WHERE name = ?",&(name,))?.into_iter().next().ok_or("Unknown watch")?;
    let mut previous = status(found)?;
    let now = berm_lang::now();
    match observe(&previous.watch) {
        Ok(snapshot) => {
            let serialized = encode(&snapshot)?;
            let old = previous.snapshot.as_ref().map(encode).transpose()?;
            let kind = if previous.snapshot.is_none() {
                Some("baseline")
            } else if old.as_ref() != Some(&serialized) {
                Some("changed")
            } else if previous.error.is_some() {
                Some("recovered")
            } else {
                None
            };
            if let Some(kind) = kind {
                record(&previous, now, kind, Some(&serialized), None)?;
            }
            rows::<()>(
                "UPDATE release_watch_watches SET attempted_at=?,succeeded_at=?,error=NULL,snapshot=? WHERE name=?",
                &(now, now, &serialized, name),
            )?;
            previous.snapshot = Some(snapshot);
            previous.succeeded_at = Some(now);
            previous.error = None;
        }
        Err(error) => {
            if previous.error.as_ref() != Some(&error.message) {
                record(&previous, now, "failed", None, Some(&error.message))?;
            }
            rows::<()>(
                "UPDATE release_watch_watches SET attempted_at=?,error=? WHERE name=?",
                &(now, &error.message, name),
            )?;
            previous.error = Some(error.message);
        }
    }
    previous.attempted_at = Some(now);
    Ok(previous)
}
/// Check up to five due watches, oldest attempt first, at most once an hour per watch.
pub fn refresh_due() -> Result<Refreshed> {
    let now = berm_lang::now();
    let due = rows::<(String,)>(
        "SELECT name FROM release_watch_watches WHERE attempted_at IS NULL OR attempted_at <= ? - ? ORDER BY attempted_at,name LIMIT 5",
        &(now, HOUR),
    )?;
    let mut result = Refreshed::default();
    for (name,) in due {
        let status = refresh(&name)?;
        result.checked += 1;
        result.failed += u64::from(status.error.is_some());
    }
    Ok(result)
}
