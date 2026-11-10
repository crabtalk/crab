use crate::{
    agent::Agent,
    conversation::Conversation,
    message::{Message, Messages},
    platform::Platform,
};
use alloc::{string::String, vec::Vec};
use berm_lang::{Result, tool};

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
