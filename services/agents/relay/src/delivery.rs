use crate::{agent::Agent, platform::Platform};
use alloc::{format, string::String, vec::Vec};
use berm_lang::{CallError, Result, Schema, caller_storage, http, rows, serde_json};

const BATCH: u64 = 5;
const MAX_ATTEMPTS: u32 = 6;
const RETRY_BASE: u64 = 60_000;
const RETRY_MAX: u64 = 3_600_000;
const TIMEOUT: u64 = 10_000;
const AUTHORIZATION: &str = "Authorization";
const CONTENT_TYPE: &str = "Content-Type";
const JSON: &str = "application/json";
const DELIVER: &str = "deliver";

#[derive(Schema, Clone, Copy)]
pub enum Reason {
    TemporaryFailure,
    AccessDenied,
    Unauthorized,
    Rejected,
    AttemptsExhausted,
}

#[derive(Schema)]
pub struct Delivery {
    pub pending: bool,
    pub failing: bool,
    pub attempts: u32,
    pub next_attempt: Option<u64>,
    pub status: Option<u16>,
    pub reason: Option<Reason>,
}

struct Wake {
    id: String,
    handle: String,
    url: String,
    secret: String,
    attempts: u32,
}

impl Delivery {
    pub fn get(handle: &str) -> Result<Option<Self>> {
        let agent = Agent::named(handle)?;
        Ok(rows::<(u8, u8, u32, u64, Option<u16>, Option<Reason>)>(
            "SELECT pending, failing, attempts, next_attempt, status, reason FROM relay_wakes WHERE id = ?",
            &(&agent.wake,),
        )?.pop().map(|(pending, failing, attempts, next, status, reason)| Self {
            pending: pending != 0,
            failing: failing != 0,
            attempts,
            next_attempt: (pending != 0 && failing == 0).then_some(next),
            status,
            reason,
        }))
    }

    pub(crate) fn enqueue(agent: &Agent) -> Result<()> {
        if let Platform::Grok(connection) = &agent.platform {
            rows::<()>(
                "INSERT INTO relay_wakes (id, handle, url, secret, next_attempt) VALUES (?, ?, ?, ?, ?)
                 ON CONFLICT(id) DO UPDATE SET pending = 1,
                    attempts = CASE WHEN reason = 'temporary_failure' THEN attempts ELSE 0 END,
                    next_attempt = CASE WHEN reason = 'temporary_failure' THEN next_attempt ELSE excluded.next_attempt END
                 WHERE NOT pending AND NOT failing",
                &(&agent.wake, &agent.handle, &connection.webhook_url, &connection.secret_id, berm_lang::now()),
            )?;
        }
        Ok(())
    }

    pub(crate) fn reset(agent: &Agent) -> Result<()> {
        rows::<()>("DELETE FROM relay_wakes WHERE id = ?", &(&agent.wake,))?;
        Ok(())
    }

    pub(crate) fn refresh(agent: &Agent, rearm: bool) -> Result<()> {
        if !matches!(agent.platform, Platform::Grok(_)) {
            return Ok(());
        }
        let active = agent.enabled
            && !caller_storage::rows::<(u8,)>(
                "SELECT 1 FROM relay_messages m
             JOIN relay_agents a ON a.id = m.recipient
             JOIN relay_conversations c ON c.id = m.conversation
             WHERE m.recipient = ? AND m.id > a.read_at AND NOT c.paused LIMIT 1",
                &(agent.id,),
            )?
            .is_empty();
        if !active {
            rows::<()>(
                "UPDATE relay_wakes SET pending = 0 WHERE id = ? AND pending",
                &(&agent.wake,),
            )?;
        } else if rearm {
            Self::enqueue(agent)?;
        }
        Ok(())
    }

    pub(crate) fn schedule() -> Result<()> {
        if let Some((due,)) = rows::<(u64,)>(
            "SELECT next_attempt FROM relay_wakes WHERE pending AND NOT failing ORDER BY next_attempt, id LIMIT 1",
            &[(); 0],
        )?.pop() {
            berm_lang::after(due.saturating_sub(berm_lang::now()), "", DELIVER, "{}")?;
        }
        Ok(())
    }

    pub fn tick() -> Result<u64> {
        if berm_lang::caller()?.is_some() || berm_lang::user()?.is_some() {
            return Err(berm_lang::deny(
                "Delivery runs only as a scheduled invocation",
            ));
        }
        let wakes = rows::<(String, String, String, String, u32)>(
            "SELECT id, handle, url, secret, attempts FROM relay_wakes
             WHERE pending AND NOT failing AND next_attempt <= ? ORDER BY next_attempt, id LIMIT ?",
            &(berm_lang::now(), BATCH),
        )?;
        let count = wakes.len() as u64;
        for (id, handle, url, secret, attempts) in wakes {
            Wake {
                id,
                handle,
                url,
                secret,
                attempts,
            }
            .send()?
            .save()?;
        }
        Self::schedule()?;
        Ok(count)
    }
}

impl Wake {
    fn send(self) -> Result<Attempt> {
        let authorization = format!("Bearer {{secret:{}}}", self.secret);
        let body = serde_json::to_vec(&serde_json::json!({
            "message": format!("@{} has new messages", self.handle),
        }))?;
        let mut attempt = Attempt {
            wake: self.id,
            delivery: Delivery {
                pending: true,
                failing: false,
                attempts: self.attempts + 1,
                next_attempt: None,
                status: None,
                reason: None,
            },
        };
        let result = http::fetch_all_literal_body(&[http::Request {
            method: "POST",
            url: &self.url,
            headers: &[(AUTHORIZATION, &authorization), (CONTENT_TYPE, JSON)],
            body: &body,
            timeout: Some(TIMEOUT),
            limit: Some(0),
        }])
        .and_then(|mut responses: Vec<_>| {
            responses
                .pop()
                .ok_or_else(|| CallError::Failed(String::from("No wake response")))?
        });
        match result {
            Ok(response) => {
                attempt.delivery.status = Some(response.status);
                match response.status {
                    200 => attempt.delivery.pending = false,
                    429 | 500..=599 => attempt.reschedule(response.retry_after()),
                    401 | 403 => attempt.fail(Reason::Unauthorized),
                    _ => attempt.fail(Reason::Rejected),
                }
            }
            Err(CallError::Refused(_)) => attempt.fail(Reason::AccessDenied),
            Err(_) => attempt.reschedule(None),
        }
        Ok(attempt)
    }
}

struct Attempt {
    wake: String,
    delivery: Delivery,
}

impl Attempt {
    fn fail(&mut self, reason: Reason) {
        self.delivery.pending = false;
        self.delivery.failing = true;
        self.delivery.reason = Some(reason);
    }

    fn reschedule(&mut self, retry_after: Option<u64>) {
        let attempts = self.delivery.attempts;
        if attempts >= MAX_ATTEMPTS {
            self.fail(Reason::AttemptsExhausted);
        } else {
            let backoff = (RETRY_BASE << (attempts - 1)).min(RETRY_MAX);
            let wait =
                retry_after.map_or(backoff, |seconds| seconds.saturating_mul(1000).max(backoff));
            self.delivery.next_attempt = Some(berm_lang::now().saturating_add(wait));
            self.delivery.reason = Some(Reason::TemporaryFailure);
        }
    }

    fn save(&self) -> Result<()> {
        let delivery = &self.delivery;
        if !delivery.pending && !delivery.failing {
            rows::<()>("DELETE FROM relay_wakes WHERE id = ?", &(&self.wake,))?;
            return Ok(());
        }
        rows::<()>(
            "UPDATE relay_wakes SET pending = ?, failing = ?, attempts = ?, next_attempt = ?, status = ?, reason = ? WHERE id = ?",
            &(
                delivery.pending,
                delivery.failing,
                delivery.attempts,
                delivery.next_attempt.unwrap_or(0),
                delivery.status,
                &delivery.reason,
                &self.wake,
            ),
        )?;
        Ok(())
    }
}
