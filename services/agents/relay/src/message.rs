use crate::{agent::Agent, conversation::Conversation, delivery::Delivery, platform::Platform};
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
    pub from: String,
    pub to: String,
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
    pub fn address(from: Option<String>, to: String, body: String, key: String) -> Result<Self> {
        let from = Agent::connected(from.as_deref())?;
        let to = Agent::named(&to)?;
        if from.id == to.id {
            return Err("Address another agent".into());
        }
        Self::validate(&body, &key)?;
        let conversation = if let Some((id,)) = berm_lang::rows::<(u64,)>(
            "SELECT id FROM relay_messages WHERE sender = ? AND request_key = ?",
            &(from.id, &key),
        )?
        .pop()
        {
            Self::get(id)?.conversation
        } else {
            Conversation::direct(from.id, to.id)?.id
        };
        Self::send(conversation, from.handle, to.handle, body, key, None)
    }

    pub fn reply(message: u64, body: String, key: String) -> Result<Self> {
        let parent = Self::get(message)?;
        Self::send(
            parent.conversation,
            parent.to,
            parent.from,
            body,
            key,
            Some(message),
        )
    }

    fn validate(body: &str, key: &str) -> Result<()> {
        if key.is_empty() || key.len() > 128 {
            return Err("Use an idempotency key of 1–128 bytes".into());
        }
        if body.trim().is_empty() || body.len() > MAX_BODY {
            return Err("Keep messages to 1–4096 bytes; link larger artifacts".into());
        }
        Ok(())
    }

    pub fn received(agent: Option<String>, cursor: Option<String>) -> Result<Page<Self>> {
        let (id, cursor) = match cursor {
            Some(cursor) => {
                let (handle, position) = cursor.split_once(':').ok_or("Invalid message cursor")?;
                (Agent::named(handle)?.id, Some(String::from(position)))
            }
            None => (Agent::connected(agent.as_deref())?.id, None),
        };
        let handle = Agent::get(id)?.handle;
        let mut page = Self::events(&handle, cursor)?;
        page.cursor = alloc::format!("{handle}:{}", page.cursor);
        for event in &mut page.events {
            event.cursor = alloc::format!("{handle}:{}", event.cursor);
        }
        Ok(page)
    }

    pub fn events(handle: &str, cursor: Option<String>) -> Result<Page<Self>> {
        let recipient = Agent::named(handle)?;
        let agent = recipient.id;
        recipient.require_key()?;
        recipient.require_enabled()?;
        if !matches!(recipient.platform, Platform::OpenAiDot(_)) {
            return Err(berm_lang::deny("Use a Dot agent for MCP event delivery"));
        }
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
        let blocked_at = berm_lang::rows::<(u64,)>(
            "SELECT m.id FROM relay_messages m JOIN relay_conversations c ON c.id = m.conversation WHERE m.recipient = ? AND m.id > ? AND c.paused = 1 ORDER BY m.id LIMIT 1",
            &(agent, after),
        )?.pop().map(|(id,)| id);
        let page = Self::page(None, Some(agent), after, None)?;
        let mut events = Vec::new();
        let mut blocked = false;
        for message in page.items {
            if blocked_at.is_some_and(|id| message.id >= id) {
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
        from: String,
        to: String,
        body: String,
        key: String,
        reply_to: Option<u64>,
    ) -> Result<Self> {
        let context = Conversation::get(conversation)?;
        let sender = Agent::named(&from)?;
        let recipient = Agent::named(&to)?;
        sender.require_key()?;
        let from = sender.id;
        let to = recipient.id;
        Self::validate(&body, &key)?;
        if let Some((id,)) = berm_lang::rows::<(u64,)>(
            "SELECT id FROM relay_messages WHERE sender = ? AND request_key = ?",
            &(from, &key),
        )?
        .pop()
        {
            let message = Self::get(id)?;
            if message.conversation != conversation
                || message.to != recipient.handle
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
        let message = Self {
            id,
            conversation,
            from: sender.handle,
            to: recipient.handle,
            body,
            reply_to,
            created_at,
        };
        Delivery::enqueue(&message)?;
        Ok(message)
    }

    pub fn get(id: u64) -> Result<Self> {
        let message = Self::stored(id)?;
        Conversation::get(message.conversation)?;
        Ok(message)
    }

    pub(crate) fn stored(id: u64) -> Result<Self> {
        let (conversation, from, to, body, reply_to, created_at) = berm_lang::rows::<(u64, String, String, String, Option<u64>, u64)>(
            "SELECT m.conversation, s.handle, r.handle, m.body, m.reply_to, m.created_at FROM relay_messages m JOIN relay_agents s ON s.id = m.sender JOIN relay_agents r ON r.id = m.recipient WHERE m.id = ?",
            &(id,),
        )?.pop().ok_or("No such message")?;
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

    pub fn inbox(handle: &str, after: u64, limit: Option<u64>) -> Result<Messages> {
        let agent = Agent::named(handle)?;
        agent.require_key()?;
        let agent = agent.id;
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
                berm_lang::rows::<(u64, u64, String, String, String, Option<u64>, u64)>(
                    "SELECT m.id, m.conversation, s.handle, r.handle, m.body, m.reply_to, m.created_at FROM relay_messages m JOIN relay_agents s ON s.id = m.sender JOIN relay_agents r ON r.id = m.recipient WHERE m.conversation = ? AND m.id > ? ORDER BY m.id LIMIT ?",
                    &(conversation, after, limit + 1),
                )?
            }
            (_, Some(recipient)) => {
                berm_lang::rows::<(u64, u64, String, String, String, Option<u64>, u64)>(
                    "SELECT m.id, m.conversation, s.handle, r.handle, m.body, m.reply_to, m.created_at FROM relay_messages m JOIN relay_agents s ON s.id = m.sender JOIN relay_agents r ON r.id = m.recipient WHERE m.recipient = ? AND m.id > ? ORDER BY m.id LIMIT ?",
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
