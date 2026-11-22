use crate::{account::Account, track};
use alloc::string::String;
use berm_lang::{
    Result, caller_storage, json, rows,
    serde::{Deserialize, Serialize},
};

const DRAIN_LIMIT: usize = 5_000;
// Even fully escaped rows fit in the default syscall buffer at this page size.
const PAGE: usize = 24;

#[derive(Deserialize, Serialize)]
#[serde(crate = "berm_lang::serde")]
struct Hit {
    id: u64,
    domain: String,
    visitor: String,
    session: String,
    kind: String,
    path: Option<String>,
    referrer: Option<String>,
    source: Option<String>,
    medium: Option<String>,
    campaign: Option<String>,
    name: Option<String>,
    value: Option<i64>,
    currency: Option<String>,
    at: u64,
    #[serde(default)]
    channel: String,
}

pub(crate) fn run(account: &Account) -> Result<()> {
    let Some(account) = &account.claim_id else {
        return Ok(());
    };
    let mut remaining = DRAIN_LIMIT;
    for (domain, ceiling) in rows::<(String, u64)>(
        "SELECT domain, (SELECT max(id) FROM buffer WHERE buffer.domain = claims.domain) AS ceiling
         FROM claims WHERE account = ?1 AND verified_at IS NOT NULL AND ceiling IS NOT NULL
         ORDER BY domain",
        &(account,),
    )? {
        let mut last = 0;
        while remaining > 0 {
            let page = remaining.min(PAGE);
            let mut hits = rows::<Hit>(
                "SELECT id, domain, visitor, session, kind, path, referrer, source, medium,
                        campaign, name, value, currency, at
                 FROM buffer WHERE domain = ?1 AND id > ?2 AND id <= ?3 ORDER BY id LIMIT ?4",
                &(&domain, last, ceiling, page),
            )?;
            let Some(hit) = hits.last() else {
                break;
            };
            last = hit.id;
            for hit in &mut hits {
                hit.channel =
                    track::channel(&hit.domain, hit.source.as_deref(), hit.referrer.as_deref());
            }
            caller_storage::rows::<()>(
                "INSERT OR IGNORE INTO hits
                 (buffer_id, domain, visitor, session, kind, path, channel, referrer, source,
                  medium, campaign, name, value, currency, at)
                 SELECT value->>'id', value->>'domain', value->>'visitor', value->>'session',
                        value->>'kind', value->>'path', value->>'channel', value->>'referrer',
                        value->>'source', value->>'medium', value->>'campaign', value->>'name',
                        value->>'value', value->>'currency', value->>'at' FROM json_each(?1)",
                &(json::to(&hits)?,),
            )?;
            remaining -= hits.len();
            if hits.len() < page || last == ceiling {
                break;
            }
        }
        // The host commits caller storage before shared storage.
        if last > 0 {
            rows::<()>(
                "DELETE FROM buffer WHERE domain = ?1 AND id <= ?2",
                &(&domain, last),
            )?;
        }
        if remaining == 0 {
            break;
        }
    }
    Ok(())
}
