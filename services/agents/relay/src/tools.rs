use crate::{
    agent::{AccessPage, Agent, Contact, Contacts},
    conversation::Conversation,
    delivery::Delivery,
    message::{Message, Messages},
    platform::Platform,
};
use alloc::{string::String, vec::Vec};
use berm_lang::{Result, abi::events::Page, tool};

/// Register your own unique handle; this connection receives access automatically. Retry the same profile safely.
#[tool]
pub fn register(
    handle: String,
    name: Option<String>,
    platform: Option<Platform>,
) -> Result<Contact> {
    Ok(Contact::from(Agent::register(handle, name, platform)?))
}

/// List this connection's registered handles, at most twenty.
#[tool(query)]
pub fn agents(after: Option<String>, limit: Option<u64>) -> Result<Vec<Contact>> {
    Ok(Agent::list(after.as_deref(), limit)?
        .into_iter()
        .map(Contact::from)
        .collect())
}

/// Change an agent's profile or connection while preserving its identity.
#[tool]
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

/// List the API key IDs permitted to act as an agent; owner sessions only.
#[tool(query)]
pub fn agent_keys(agent: String) -> Result<Vec<String>> {
    Agent::keys(&agent)
}

/// Allow an API key ID to act as this agent; owner sessions only.
#[tool]
pub fn grant_agent(agent: String, key_id: String) -> Result<()> {
    Agent::grant_key(&agent, key_id)
}

/// Remove an API key's agent grant; owner sessions only.
#[tool]
pub fn revoke_agent(agent: String, key_id: String) -> Result<()> {
    Agent::revoke_key(&agent, key_id)
}

/// Open a conversation between 2–16 of this account's agents.
#[tool]
pub fn open(members: Vec<String>, budget: Option<u64>) -> Result<Conversation> {
    Conversation::open(members, budget)
}

/// Pause or resume new messages in a conversation.
#[tool]
pub fn pause(conversation: u64, paused: bool) -> Result<Conversation> {
    Conversation::pause(conversation, paused)
}

/// Persist a message; retry with the same sender and key to obtain its original receipt.
#[tool]
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

/// Read conversation messages after a cursor, at most ten.
#[tool(query)]
pub fn read(conversation: u64, after: Option<u64>, limit: Option<u64>) -> Result<Messages> {
    Message::read(conversation, after.unwrap_or(0), limit)
}

/// Read messages addressed to an agent after a cursor, at most ten.
#[tool(query)]
pub fn inbox(agent: String, after: Option<u64>, limit: Option<u64>) -> Result<Messages> {
    Message::inbox(&agent, after.unwrap_or(0), limit)
}

/// Receive committed messages addressed to an enabled agent.
#[tool(events = "message.created")]
pub fn message_events(agent: String, cursor: Option<String>) -> Result<Page<Message>> {
    Message::events(&agent, cursor)
}

/// Read the latest Grok delivery state for a message in an accessible conversation.
#[tool(query)]
pub fn delivery(message: u64) -> Result<Option<Delivery>> {
    Delivery::get(message)
}

/// Retry an unsuccessful Grok delivery with the agent's current connection; owner sessions only.
#[tool]
pub fn retry_delivery(message: u64) -> Result<Delivery> {
    Delivery::retry(message)
}

/// Attempt up to five due Grok deliveries.
#[tool]
pub fn deliver() -> Result<u64> {
    Delivery::tick()
}

/// List the handles this account can address.
#[tool(query)]
pub fn contacts(after: Option<String>, limit: Option<u64>) -> Result<Contacts> {
    Agent::contacts(after.as_deref(), limit)
}

/// Review agent handle access for a connection; account-owner approval only.
#[tool(query)]
pub fn access(
    key_id: Option<String>,
    after: Option<String>,
    limit: Option<u64>,
) -> Result<AccessPage> {
    Agent::access(key_id.as_deref(), after.as_deref(), limit)
}

/// Approve a connection's existing handles; account owners only.
#[tool]
pub fn connect(key_id: String, agents: Vec<String>) -> Result<Vec<Contact>> {
    Agent::connect(key_id, agents)
}

/// Message an agent by handle. The conversation is opened automatically; omit from when one identity is connected.
#[tool]
pub fn message(to: String, body: String, key: String, from: Option<String>) -> Result<Message> {
    Message::address(from, to, body, key)
}

/// Reply to a received message in its original conversation as its recipient.
#[tool]
pub fn reply(message: u64, body: String, key: String) -> Result<Message> {
    Message::reply(message, body, key)
}

/// Read incoming messages; omit agent when one identity is connected.
#[tool(query)]
pub fn messages(agent: Option<String>, after: Option<u64>, limit: Option<u64>) -> Result<Messages> {
    Message::inbox(
        &Agent::connected(agent.as_deref())?.handle,
        after.unwrap_or(0),
        limit,
    )
}

/// Receive incoming messages for a connected assistant; omit agent when one identity is connected.
#[tool(events = "message.received")]
pub fn receive_events(agent: Option<String>, cursor: Option<String>) -> Result<Page<Message>> {
    Message::received(agent, cursor)
}
