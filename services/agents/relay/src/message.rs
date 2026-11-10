use crate::{agent::Agent, conversation::Conversation};
use alloc::{
    string::{String, ToString},
    vec::Vec,
};
use berm_lang::{
    Result, Schema,
    abi::events::{Event, Page},
};

const MAX_BODY: usize = 4096;

#[derive(Schema)]
pub struct Message {
    pub id: u64,
    pub conversation: u64,
    pub from: u64,
    pub to: u64,
    pub body: String,
    pub reply_to: Option<u64>,
    pub created_at: u64,
}

#[derive(Schema)]
pub struct Messages {
    pub items: Vec<Message>,
    pub next: u64,
    pub has_more: bool,
}

impl Message {
    pub fn events(agent: u64, cursor: Option<String>) -> Result<Page<Self>> {
        Agent::get(agent)?.require_enabled()?;
        let Some(cursor) = cursor else {
            let (head,) = berm_lang::rows::<(u64,)>(
                "SELECT COALESCE(MAX(id), 0) FROM relay_messages WHERE recipient = ?",
                &(agent,),
            )?
            .pop()
            .ok_or("No message checkpoint")?;
            return Ok(Page {
                events: Vec::new(),
                cursor: head.to_string(),
                has_more: false,
            });
        };
        let after = cursor
            .parse::<u64>()
            .map_err(|_| "Invalid message cursor")?;
        let page = Self::page(None, Some(agent), after, None)?;
        let mut events = Vec::new();
        let mut blocked = false;
        for message in page.items {
            if Conversation::get(message.conversation)?.paused {
                blocked = true;
                break;
            }
            events.push(Event {
                id: message.id.to_string(),
                cursor: message.id.to_string(),
                at: message.created_at,
                data: message,
            });
        }
        let cursor = events
            .last()
            .map_or_else(|| after.to_string(), |event| event.cursor.clone());
        Ok(Page {
            events,
            cursor,
            has_more: !blocked && page.has_more,
        })
    }

    pub fn send(
        conversation: u64,
        from: u64,
        to: u64,
        body: String,
        key: String,
        reply_to: Option<u64>,
    ) -> Result<Self> {
        let context = Conversation::get(conversation)?;
        // Authorize before resolving a sender's idempotency key.
        Agent::get(from)?;
        if key.is_empty() || key.len() > 128 {
            return Err("Use an idempotency key of 1–128 bytes".into());
        }
        if body.trim().is_empty() || body.len() > MAX_BODY {
            return Err("Keep messages to 1–4096 bytes; link larger artifacts".into());
        }
        if let Some((id,)) = berm_lang::rows::<(u64,)>(
            "SELECT id FROM relay_messages WHERE sender = ? AND request_key = ?",
            &(from, &key),
        )?
        .pop()
        {
            let message = Self::get(id)?;
            if message.conversation != conversation
                || message.to != to
                || message.body != body
                || message.reply_to != reply_to
            {
                return Err("This idempotency key already identifies a different message".into());
            }
            return Ok(message);
        }
        context.require_member(from)?;
        context.require_member(to)?;
        if from == to {
            return Err("Address another agent in the conversation".into());
        }
        if context.paused {
            return Err(berm_lang::deny("This conversation is paused"));
        }
        if context.sent >= context.budget {
            return Err(berm_lang::deny(
                "This conversation has reached its message budget",
            ));
        }
        if let Some(reply_to) = reply_to {
            let parent = Self::get(reply_to)?;
            if parent.conversation != conversation {
                return Err("Reply to a message in the same conversation".into());
            }
        }
        let created_at = berm_lang::now();
        let (id,) = berm_lang::rows::<(u64,)>(
            "INSERT INTO relay_messages (conversation, sender, recipient, body, request_key, reply_to, created_at) VALUES (?, ?, ?, ?, ?, ?, ?) RETURNING id",
            &(conversation, from, to, &body, &key, reply_to, created_at),
        )?.pop().ok_or("The message was not stored")?;
        berm_lang::rows::<()>(
            "UPDATE relay_conversations SET sent = sent + 1 WHERE id = ?",
            &(conversation,),
        )?;
        Ok(Self {
            id,
            conversation,
            from,
            to,
            body,
            reply_to,
            created_at,
        })
    }

    pub fn get(id: u64) -> Result<Self> {
        let (conversation, from, to, body, reply_to, created_at) = berm_lang::rows::<(u64, u64, u64, String, Option<u64>, u64)>(
            "SELECT conversation, sender, recipient, body, reply_to, created_at FROM relay_messages WHERE id = ?",
            &(id,),
        )?.pop().ok_or("No such message")?;
        Conversation::get(conversation)?;
        Ok(Self {
            id,
            conversation,
            from,
            to,
            body,
            reply_to,
            created_at,
        })
    }

    pub fn read(conversation: u64, after: u64, limit: Option<u64>) -> Result<Messages> {
        Conversation::get(conversation)?;
        Self::page(Some(conversation), None, after, limit)
    }

    pub fn inbox(agent: u64, after: u64, limit: Option<u64>) -> Result<Messages> {
        Agent::get(agent)?;
        Self::page(None, Some(agent), after, limit)
    }

    fn page(
        conversation: Option<u64>,
        recipient: Option<u64>,
        after: u64,
        limit: Option<u64>,
    ) -> Result<Messages> {
        let limit = limit.unwrap_or(10).clamp(1, 10);
        let mut found = match (conversation, recipient) {
            (Some(conversation), _) => {
                berm_lang::rows::<(u64, u64, u64, u64, String, Option<u64>, u64)>(
                    "SELECT id, conversation, sender, recipient, body, reply_to, created_at FROM relay_messages WHERE conversation = ? AND id > ? ORDER BY id LIMIT ?",
                    &(conversation, after, limit + 1),
                )?
            }
            (_, Some(recipient)) => {
                berm_lang::rows::<(u64, u64, u64, u64, String, Option<u64>, u64)>(
                    "SELECT id, conversation, sender, recipient, body, reply_to, created_at FROM relay_messages WHERE recipient = ? AND id > ? ORDER BY id LIMIT ?",
                    &(recipient, after, limit + 1),
                )?
            }
            _ => return Err("Select a conversation or recipient".into()),
        };
        let has_more = found.len() as u64 > limit;
        found.truncate(limit as usize);
        let items: Vec<_> = found
            .into_iter()
            .map(
                |(id, conversation, from, to, body, reply_to, created_at)| Self {
                    id,
                    conversation,
                    from,
                    to,
                    body,
                    reply_to,
                    created_at,
                },
            )
            .collect();
        Ok(Messages {
            next: items.last().map_or(after, |message| message.id),
            items,
            has_more,
        })
    }
}
