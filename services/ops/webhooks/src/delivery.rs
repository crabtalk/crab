use alloc::{string::String, vec::Vec};
use berm_lang::{
    Result, Schema,
    serde_json::{self, Value},
};

const MAX_PAYLOAD: usize = 16 * 1024;
const MAX_DELIVERIES: u64 = 10_000;
const RETENTION: u64 = 30 * 86_400_000;

#[derive(Schema)]
pub struct Delivery {
    pub seq: u64,
    pub source: String,
    pub id: String,
    pub payload: Value,
    pub received_at: u64,
    pub acknowledged: bool,
}

#[derive(Schema)]
pub struct Receipt {
    pub seq: u64,
    pub duplicate: bool,
}

#[derive(Schema)]
pub struct Summary {
    pub seq: u64,
    pub source: String,
    pub id: String,
    pub received_at: u64,
    pub acknowledged: bool,
}

#[derive(Schema)]
pub struct Deliveries {
    pub items: Vec<Summary>,
    pub next: u64,
    pub has_more: bool,
}

impl Delivery {
    pub fn receive(source: String, id: String, payload: Value) -> Result<Receipt> {
        if source.trim().is_empty() || source.len() > 64 || id.trim().is_empty() || id.len() > 256 {
            return Err("Use a source of 1–64 bytes and an event id of 1–256 bytes".into());
        }
        let payload = serde_json::to_string(&payload)?;
        if payload.len() > MAX_PAYLOAD {
            return Err("The event payload exceeds 16 KiB".into());
        }
        if let Some((seq, held)) = berm_lang::rows::<(u64, String)>(
            "SELECT seq, payload FROM inbox_deliveries WHERE source = ? AND id = ?",
            &(&source, &id),
        )?
        .pop()
        {
            if held != payload {
                return Err("This event id already has a different payload".into());
            }
            return Ok(Receipt {
                seq,
                duplicate: true,
            });
        }
        let (count,) =
            berm_lang::rows::<(u64,)>("SELECT COUNT(*) FROM inbox_deliveries", &[0u8; 0])?
                .pop()
                .unwrap_or_default();
        if count >= MAX_DELIVERIES {
            return Err(
                "The inbox holds 10000 events; prune or delete events before retrying".into(),
            );
        }
        let (seq,) = berm_lang::rows::<(u64,)>("INSERT INTO inbox_deliveries (source, id, payload, received_at) VALUES (?, ?, ?, ?) RETURNING seq", &(&source, &id, &payload, berm_lang::now()))?.pop().ok_or("The event was not stored")?;
        Ok(Receipt {
            seq,
            duplicate: false,
        })
    }

    pub fn get(seq: u64) -> Result<Option<Self>> {
        berm_lang::rows::<(String, String, String, u64, u8)>("SELECT source, id, payload, received_at, acknowledged FROM inbox_deliveries WHERE seq = ?", &(seq,))?
            .pop().map(|(source, id, payload, received_at, acknowledged)| Ok(Self { seq, source, id, payload: serde_json::from_str(&payload)?, received_at, acknowledged: acknowledged != 0 })).transpose()
    }

    pub fn events(after: u64, pending_only: bool, limit: Option<u64>) -> Result<Deliveries> {
        let limit = limit.unwrap_or(20).clamp(1, 100);
        let mut found = berm_lang::rows::<(u64, String, String, u64, u8)>(
            "SELECT seq, source, id, received_at, acknowledged FROM inbox_deliveries WHERE seq > ? AND (?2 = 0 OR acknowledged = 0) ORDER BY seq LIMIT ?",
            &(after, pending_only, limit + 1),
        )?;
        let has_more = found.len() as u64 > limit;
        found.truncate(limit as usize);
        let items: Vec<_> = found
            .into_iter()
            .map(|(seq, source, id, received_at, acknowledged)| Summary {
                seq,
                source,
                id,
                received_at,
                acknowledged: acknowledged != 0,
            })
            .collect();
        Ok(Deliveries {
            next: items.last().map_or(after, |item| item.seq),
            items,
            has_more,
        })
    }

    pub fn acknowledge(seq: u64) -> Result<bool> {
        Ok(!berm_lang::rows::<(u64,)>("UPDATE inbox_deliveries SET acknowledged = 1 WHERE seq = ? AND acknowledged = 0 RETURNING seq", &(seq,))?.is_empty())
    }

    pub fn delete(seq: u64) -> Result<bool> {
        Ok(!berm_lang::rows::<(u64,)>(
            "DELETE FROM inbox_deliveries WHERE seq = ? RETURNING seq",
            &(seq,),
        )?
        .is_empty())
    }

    pub fn prune() -> Result<()> {
        berm_lang::rows::<()>(
            "DELETE FROM inbox_deliveries WHERE received_at < ?",
            &(berm_lang::now().saturating_sub(RETENTION),),
        )?;
        Ok(())
    }
}
