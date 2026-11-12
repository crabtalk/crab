use crate::{account::Account, agent::Agent, message::Message, platform::Platform};
use alloc::{format, string::String, vec::Vec};
use berm_lang::{CallError, Result, Schema, http, serde::Serialize, serde_json};

const BATCH: u64 = 5;
const MAX_ATTEMPTS: u32 = 6;
const RETRY_BASE: u64 = 60_000;
const RETRY_MAX: u64 = 3_600_000;
const TIMEOUT: u64 = 10_000;
const AUTHORIZATION: &str = "Authorization";
const CONTENT_TYPE: &str = "Content-Type";
const JSON: &str = "application/json";
const EVENT: &str = "message.created";
const DELIVER: &str = "deliver";

type Row = (u64, State, u32, u64, Option<u16>, Option<Reason>);

#[derive(Schema, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Pending,
    Accepted,
    Failed,
    Cancelled,
}

#[derive(Schema, Clone, Copy)]
pub enum Reason {
    TemporaryFailure,
    AccessDenied,
    Unauthorized,
    Rejected,
    AttemptsExhausted,
    ConnectionChanged,
}

#[derive(Schema)]
pub struct Delivery {
    pub message: u64,
    pub state: State,
    pub attempts: u32,
    pub next_attempt: Option<u64>,
    pub status: Option<u16>,
    pub reason: Option<Reason>,
}

#[derive(Serialize)]
#[serde(crate = "berm_lang::serde")]
struct Payload<'a> {
    event: &'static str,
    message: &'a Message,
}

impl Delivery {
    pub fn get(message: u64) -> Result<Option<Self>> {
        Message::get(message)?;
        Self::stored(message)
    }

    fn stored(message: u64) -> Result<Option<Self>> {
        Ok(berm_lang::rows::<Row>(
            "SELECT message, state, attempts, next_attempt, status, reason FROM relay_deliveries WHERE message = ?",
            &(message,),
        )?.pop().map(|(message, state, attempts, next, status, reason)| Self {
            message, state, attempts,
            next_attempt: (state == State::Pending).then_some(next),
            status, reason,
        }))
    }

    pub(crate) fn enqueue(message: &Message) -> Result<()> {
        let recipient = Agent::recipient(message.id)?;
        if matches!(recipient.platform, Platform::Grok(_)) {
            berm_lang::rows::<()>(
                "INSERT INTO relay_deliveries (message, next_attempt) VALUES (?, ?)",
                &(message.id, berm_lang::now()),
            )?;
            Self::schedule()?;
        }
        Ok(())
    }

    pub fn retry(message: u64) -> Result<Self> {
        Account::require_owner()?;
        let message = Message::get(message)?;
        let recipient = Agent::recipient(message.id)?;
        recipient.require_enabled()?;
        let Platform::Grok(connection) = &recipient.platform else {
            return Err("This agent uses MCP event delivery".into());
        };
        connection.validate()?;
        let delivery = Self::stored(message.id)?.ok_or("No Grok delivery for this message")?;
        if delivery.state == State::Accepted || delivery.state == State::Pending {
            return Err("Retry only a failed or cancelled delivery".into());
        }
        berm_lang::rows::<()>(
            "UPDATE relay_deliveries SET state = 'pending', attempts = 0, next_attempt = ?, status = NULL, reason = NULL WHERE message = ?",
            &(berm_lang::now(), message.id),
        )?;
        Self::schedule()?;
        Self::stored(message.id)?.ok_or_else(|| "No delivery record".into())
    }

    fn queued(limit: u64) -> Result<Vec<(u64, u64)>> {
        Ok(berm_lang::rows::<(u64, u64)>(
            "SELECT d.message, d.next_attempt FROM relay_deliveries d JOIN relay_messages m ON m.id = d.message JOIN relay_agents a ON a.id = m.recipient JOIN relay_conversations c ON c.id = m.conversation WHERE d.state = 'pending' AND a.enabled = 1 AND c.paused = 0 AND NOT EXISTS (SELECT 1 FROM relay_deliveries earlier JOIN relay_messages prior ON prior.id = earlier.message WHERE earlier.state = 'pending' AND prior.recipient = m.recipient AND prior.id < m.id) ORDER BY d.next_attempt, d.message LIMIT ?",
            &(limit,),
        )?)
    }

    pub(crate) fn schedule() -> Result<()> {
        if let Some((_, due)) = Self::queued(1)?.first() {
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
        let mut attempted = 0;
        for (id, due) in Self::queued(BATCH)? {
            if due > berm_lang::now() {
                break;
            }
            let mut delivery = Self::stored(id)?.ok_or("No delivery record")?;
            let message = Message::stored(id)?;
            let recipient = Agent::recipient(message.id)?;
            let Platform::Grok(connection) = recipient.platform else {
                delivery.state = State::Cancelled;
                delivery.reason = Some(Reason::ConnectionChanged);
                delivery.save()?;
                continue;
            };
            let authorization = format!("Bearer {{secret:{}}}", connection.secret_id);
            let body = serde_json::to_vec(&Payload {
                event: EVENT,
                message: &message,
            })?;
            delivery.attempts += 1;
            let result = http::fetch_all_literal_body(&[http::Request {
                method: "POST",
                url: &connection.webhook_url,
                headers: &[(AUTHORIZATION, &authorization), (CONTENT_TYPE, JSON)],
                body: &body,
                timeout: Some(TIMEOUT),
                limit: Some(0),
            }])
            .and_then(|mut responses| {
                responses
                    .pop()
                    .ok_or_else(|| CallError::Failed(String::from("No delivery response")))?
            });
            match result {
                Ok(response) => {
                    delivery.status = Some(response.status);
                    match response.status {
                        200 => {
                            delivery.state = State::Accepted;
                            delivery.reason = None;
                            delivery.next_attempt = None;
                        }
                        429 | 500..=599 => delivery.reschedule(response.retry_after()),
                        401 | 403 => {
                            delivery.fail(Reason::Unauthorized);
                        }
                        _ => {
                            delivery.fail(Reason::Rejected);
                        }
                    }
                }
                Err(CallError::Refused(_)) => {
                    delivery.status = None;
                    delivery.fail(Reason::AccessDenied);
                }
                Err(_) => {
                    delivery.status = None;
                    delivery.reschedule(None);
                }
            }
            delivery.save()?;
            attempted += 1;
        }
        Self::schedule()?;
        Ok(attempted)
    }

    fn fail(&mut self, reason: Reason) {
        self.state = State::Failed;
        self.reason = Some(reason);
        self.next_attempt = None;
    }

    fn reschedule(&mut self, retry_after: Option<u64>) {
        if self.attempts >= MAX_ATTEMPTS {
            self.fail(Reason::AttemptsExhausted);
        } else {
            let backoff = (RETRY_BASE << (self.attempts - 1)).min(RETRY_MAX);
            let wait =
                retry_after.map_or(backoff, |seconds| seconds.saturating_mul(1000).max(backoff));
            self.next_attempt = Some(berm_lang::now().saturating_add(wait));
            self.reason = Some(Reason::TemporaryFailure);
        }
    }

    fn save(&self) -> Result<()> {
        berm_lang::rows::<()>(
            "UPDATE relay_deliveries SET state = ?, attempts = ?, next_attempt = ?, status = ?, reason = ? WHERE message = ?",
            &(
                &self.state,
                self.attempts,
                self.next_attempt.unwrap_or(0),
                self.status,
                &self.reason,
                self.message,
            ),
        )?;
        Ok(())
    }
}
