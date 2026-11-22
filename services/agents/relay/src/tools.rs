use crate::{
    agent::{Agent, Contact, Contacts},
    conversation::Conversation,
    delivery::Delivery,
    message::{Message, Messages},
    platform::Platform,
};
use alloc::{string::String, vec::Vec};
use berm_lang::{Result, abi::events::Page, tool};

/// Register agent
///
/// Register a handle in this account. Retry the same profile safely.
#[tool(
    access = "accounts",
    catalog = "relay.register",
    budget(cpu = 100000, read = 2000, written = 10, memory = 16)
)]
pub fn register(
    handle: String,
    name: Option<String>,
    platform: Option<Platform>,
) -> Result<Contact> {
    Ok(Contact::from(Agent::register(handle, name, platform)?))
}

/// List agents
///
/// List this account's registered handles, at most twenty.
#[tool(
    query,
    access = "accounts",
    catalog = "relay.agents",
    budget(cpu = 100000, read = 2000, written = 10, memory = 16)
)]
pub fn agents(after: Option<String>, limit: Option<u64>) -> Result<Vec<Contact>> {
    Ok(Agent::list(after.as_deref(), limit)?
        .into_iter()
        .map(Contact::from)
        .collect())
}

/// Update agent
///
/// Change an agent's profile or connection while preserving its identity.
#[tool(
    access = "accounts",
    catalog = "relay.update-agent",
    budget(cpu = 100000, read = 2000, written = 10, memory = 16)
)]
pub fn update_agent(
    handle: String,
    name: Option<String>,
    platform: Option<Platform>,
    enabled: Option<bool>,
) -> Result<Contact> {
    Ok(Contact::from(Agent::update(
        &handle, name, platform, enabled,
    )?))
}

/// Open conversation
///
/// Open a conversation between 2–16 of this account's agents.
#[tool(
    access = "accounts",
    catalog = "relay.open",
    budget(cpu = 100000, read = 2000, written = 100, memory = 16)
)]
pub fn open(members: Vec<String>, budget: Option<u64>) -> Result<Conversation> {
    Conversation::open(members, budget)
}

/// Pause conversation
///
/// Pause or resume new messages in a conversation.
#[tool(
    access = "accounts",
    catalog = "relay.pause",
    budget(cpu = 100000, read = 10000, written = 50, memory = 16)
)]
pub fn pause(conversation: u64, paused: bool) -> Result<Conversation> {
    Conversation::pause(conversation, paused)
}

/// Send message
///
/// Persist a message; retry with the same sender and key to obtain its original receipt.
#[tool(
    access = "accounts",
    catalog = "relay.send",
    budget(cpu = 100000, read = 2000, written = 10, memory = 16)
)]
pub fn send(
    conversation: u64,
    from: String,
    to: String,
    body: String,
    key: String,
    reply_to: Option<u64>,
) -> Result<Message> {
    Message::send(conversation, from, to, body, key, reply_to)
}

/// Read conversation
///
/// Read conversation messages after a cursor, at most ten.
#[tool(
    query,
    access = "accounts",
    catalog = "relay.read",
    budget(cpu = 100000, read = 2000, written = 10, memory = 16)
)]
pub fn read(conversation: u64, after: Option<u64>, limit: Option<u64>) -> Result<Messages> {
    Message::read(conversation, after.unwrap_or(0), limit)
}

/// Read agent inbox
///
/// Read up to ten unread messages. Pass the last processed message id as after to acknowledge it.
#[tool(
    access = "accounts",
    catalog = "relay.inbox",
    budget(cpu = 100000, read = 2000, written = 10, memory = 16)
)]
pub fn inbox(agent: String, after: Option<u64>, limit: Option<u64>) -> Result<Messages> {
    Message::inbox(&agent, after, limit)
}

/// Agent message events
///
/// Receive committed messages addressed to an enabled agent.
#[tool(
    events = "message.created",
    access = "accounts",
    catalog = "relay.events",
    budget(cpu = 100000, read = 2000, written = 10, memory = 16)
)]
pub fn message_events(agent: String, cursor: Option<String>) -> Result<Page<Message>> {
    Message::events(&agent, cursor)
}

/// Grok connection delivery
///
/// Read a Grok agent’s pending wake or connection failure.
#[tool(
    query,
    access = "accounts",
    catalog = "relay.delivery",
    budget(cpu = 100000, read = 2000, written = 10, memory = 16)
)]
pub fn delivery(agent: String) -> Result<Option<Delivery>> {
    Delivery::get(&agent)
}

/// Send due Grok wakes
///
/// Attempt up to five due Grok wakes.
#[tool(
    access = "internal",
    budget(cpu = 100000, read = 10000, written = 50, memory = 16)
)]
pub fn deliver() -> Result<u64> {
    Delivery::tick()
}

/// Agent contacts
///
/// List the handles this account can address.
#[tool(
    query,
    access = "accounts",
    catalog = "relay.contacts",
    budget(cpu = 100000, read = 10000, written = 10, memory = 16)
)]
pub fn contacts(after: Option<String>, limit: Option<u64>) -> Result<Contacts> {
    Agent::contacts(after.as_deref(), limit)
}

/// Message an agent
///
/// Message an agent by handle. The conversation is opened automatically; omit from when the account has one agent.
#[tool(
    access = "accounts",
    catalog = "relay.message",
    budget(cpu = 100000, read = 10000, written = 100, memory = 16)
)]
pub fn message(to: String, body: String, key: String, from: Option<String>) -> Result<Message> {
    Message::address(from, to, body, key)
}

/// Reply to an agent
///
/// Reply to a received message in its original conversation as its recipient.
#[tool(
    access = "accounts",
    catalog = "relay.reply",
    budget(cpu = 100000, read = 10000, written = 100, memory = 16)
)]
pub fn reply(message: u64, body: String, key: String) -> Result<Message> {
    Message::reply(message, body, key)
}

/// Incoming messages
///
/// Read incoming messages; omit agent when the account has one agent.
#[tool(
    query,
    access = "accounts",
    catalog = "relay.messages",
    budget(cpu = 100000, read = 10000, written = 10, memory = 16)
)]
pub fn messages(agent: Option<String>, after: Option<u64>, limit: Option<u64>) -> Result<Messages> {
    Message::incoming(
        &Agent::connected(agent.as_deref())?.handle,
        after.unwrap_or(0),
        limit,
    )
}

/// Incoming agent events
///
/// Receive incoming messages for an assistant; omit agent when the account has one agent.
#[tool(
    events = "message.received",
    access = "accounts",
    catalog = "relay.receive",
    budget(cpu = 100000, read = 10000, written = 10, memory = 16)
)]
pub fn receive_events(agent: Option<String>, cursor: Option<String>) -> Result<Page<Message>> {
    Message::received(agent, cursor)
}
