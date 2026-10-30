//! What a host's fleet pulls of its written calls and logs: those past the
//! cursors it stored, the ones up to them deleted first. Replay artifacts
//! are deleted only when the fleet opts in to pulling them.

use crate::{
    record::{calls, logs},
    state::store::Store,
};
use anyhow::Result;
use berm_api::host::{RecordedCall, RecordedLog, Records, RecordsPage};
use rusqlite::{OptionalExtension, params};
use std::collections::HashMap;

/// Bound a page of tapes; one tape alone may fill the page.
const REPLAY_BYTES: usize = 8 << 20;

/// Delete the written calls up to `page.calls`, with the calls they made, and
/// the logs up to `page.logs` emitted in no call or in one up to
/// `page.calls`; then answer the calls past `page.calls` and below `floor`,
/// and the logs past `page.logs`, at most `page.limit` of each, oldest first.
/// One transaction.
pub(crate) async fn page(store: &Store, page: RecordsPage, floor: u64) -> Result<Records> {
    store
        .run(move |conn| {
            let tx = conn.transaction()?;
            let (stored_calls, stored_logs) = (page.calls as i64, page.logs as i64);
            if page.replays {
                let mut dropped: HashMap<(String, String), i64> = HashMap::new();
                {
                    let mut statement = tx.prepare_cached(
                        "DELETE FROM service_replays WHERE call <= ?1
                         RETURNING scope, name, COALESCE(LENGTH(tape), 0)",
                    )?;
                    let mut rows = statement.query(params![stored_calls])?;
                    while let Some(row) = rows.next()? {
                        *dropped.entry((row.get(0)?, row.get(1)?)).or_default() += row.get::<_, i64>(2)?;
                    }
                }
                for ((scope, name), bytes) in dropped {
                    tx.execute(
                        "UPDATE services SET tape_bytes = tape_bytes - ?3 WHERE scope = ?1 AND name = ?2",
                        params![scope, name, bytes],
                    )?;
                }
            }
            tx.execute(
                "DELETE FROM service_nested_calls WHERE call <= ?1",
                params![stored_calls],
            )?;
            tx.execute(
                "DELETE FROM service_calls WHERE id <= ?1",
                params![stored_calls],
            )?;
            logs::delete(
                &tx,
                "id <= ?1 AND (call IS NULL OR call <= ?2)",
                params![stored_logs, stored_calls],
            )?;
            let mut recorded = tx
                .prepare_cached(&format!(
                    "SELECT {COLUMNS}, c.scope, c.name FROM service_calls c
                      WHERE c.id > ?1 AND c.id < ?2
                      ORDER BY c.id LIMIT ?3",
                    COLUMNS = calls::COLUMNS
                ))?
                .query_map(params![stored_calls, floor as i64, page.limit], |row| {
                    Ok(RecordedCall {
                        call: calls::call(row)?,
                        scope: row.get(9)?,
                        service: row.get(10)?,
                        nested: Vec::new(),
                        input: None,
                        tape: None,
                    })
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            let mut bytes = 0;
            let mut more = false;
            let mut count = 0;
            for call in &mut recorded {
                if page.replays {
                    let replay: Option<(Option<String>, Option<Vec<u8>>)> = tx.query_row(
                        "SELECT input, tape FROM service_replays WHERE scope = ?1 AND name = ?2 AND call = ?3",
                        params![call.scope, call.service, call.call.id as i64],
                        |row| Ok((row.get(0)?, row.get(1)?)),
                    ).optional()?;
                    if let Some((input, tape)) = replay {
                        let size = input.as_ref().map_or(0, String::len) + tape.as_ref().map_or(0, Vec::len);
                        if count > 0 && bytes + size > REPLAY_BYTES {
                            more = true;
                            break;
                        }
                        bytes += size;
                        call.input = input.map(|input| {
                            serde_json::from_str(&input).map_err(|error| {
                                rusqlite::Error::FromSqlConversionFailure(
                                    0, rusqlite::types::Type::Text, Box::new(error),
                                )
                            })
                        }).transpose()?;
                        call.tape = tape;
                    }
                }
                call.nested = calls::made(&tx, &call.scope, &call.service, call.call.id)?;
                count += 1;
            }
            recorded.truncate(count);
            let logs = tx
                .prepare_cached(&format!(
                    "SELECT {COLUMNS}, id, scope, name FROM service_logs
                      WHERE id > ?1
                      ORDER BY id LIMIT ?2",
                    COLUMNS = logs::COLUMNS
                ))?
                .query_map(params![stored_logs, page.limit], |row| {
                    Ok(RecordedLog {
                        log: logs::logged(row)?,
                        id: row.get::<_, i64>(5)? as u64,
                        scope: row.get(6)?,
                        service: row.get(7)?,
                    })
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            tx.commit()?;
            Ok(Records {
                calls: recorded,
                logs,
                more,
            })
        })
        .await
}
