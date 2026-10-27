//! What a host's fleet pulls of its written calls and logs: those past the
//! cursors it stored, the ones up to them deleted first. Replays are never
//! pulled nor deleted here.

use crate::{
    record::{calls, logs},
    state::store::Store,
};
use anyhow::Result;
use berm_api::host::{RecordedCall, RecordedLog, Records, RecordsPage};
use rusqlite::params;

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
                    })
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            for call in &mut recorded {
                call.nested = calls::made(&tx, &call.scope, &call.service, call.call.id)?;
            }
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
            })
        })
        .await
}
