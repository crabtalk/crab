use crate::{
    agent::Agent,
    conversation::Conversation,
    delivery::Delivery,
    message::{Message, Messages},
    platform::Platform,
};
use alloc::{string::String, vec::Vec};
use berm_lang::{Result, abi::events::Page, tool};

/// Register an existing agent under the authenticated account.
#[tool]
pub fn register(name: String, platform: Platform) -> Result<Agent> {
    Agent::register(name, platform)
}

/// List this account's agents after an id, at most twenty.
#[tool(query)]
pub fn agents(after: Option<u64>, limit: Option<u64>) -> Result<Vec<Agent>> {
    Agent::list(after.unwrap_or(0), limit)
}

/// Change an agent's profile or connection while preserving its identity.
#[tool]
pub fn update_agent(
    id: u64,
    name: Option<String>,
    platform: Option<Platform>,
    enabled: Option<bool>,
) -> Result<Agent> {
    Agent::update(id, name, platform, enabled)
}

/// List the API key IDs permitted to act as an agent; owner sessions only.
#[tool(query)]
pub fn agent_keys(agent: u64) -> Result<Vec<String>> {
    Agent::keys(agent)
}

/// Allow an API key ID to act as this agent; owner sessions only.
#[tool]
pub fn grant_agent(agent: u64, key_id: String) -> Result<()> {
    Agent::grant_key(agent, key_id)
}

/// Remove an API key's agent grant; owner sessions only.
#[tool]
pub fn revoke_agent(agent: u64, key_id: String) -> Result<()> {
    Agent::revoke_key(agent, key_id)
}

/// Open a conversation between 2–16 of this account's agents.
#[tool]
pub fn open(members: Vec<u64>, budget: Option<u64>) -> Result<Conversation> {
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
    from: u64,
    to: u64,
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
pub fn inbox(agent: u64, after: Option<u64>, limit: Option<u64>) -> Result<Messages> {
    Message::inbox(agent, after.unwrap_or(0), limit)
}

/// Receive committed messages addressed to an enabled agent.
#[tool(events = "message.created")]
pub fn message_events(agent: u64, cursor: Option<String>) -> Result<Page<Message>> {
    Message::events(agent, cursor)
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
