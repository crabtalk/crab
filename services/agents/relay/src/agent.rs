use crate::{account, delivery::Delivery, platform::Platform};
use alloc::{string::String, vec::Vec};
use berm_lang::{Result, Schema, caller_storage, serde_json};

pub struct Agent {
    pub(crate) id: u64,
    pub(crate) wake: String,
    pub handle: String,
    pub name: String,
    pub platform: Platform,
    pub enabled: bool,
}

impl Agent {
    pub fn register(
        handle: String,
        name: Option<String>,
        platform: Option<Platform>,
    ) -> Result<Self> {
        let handle = Self::handle(&handle)?;
        if let Some((id,)) = caller_storage::rows::<(u64,)>(
            "SELECT id FROM relay_agents WHERE handle = ?",
            &(&handle,),
        )?
        .pop()
        {
            let agent = Self::get(id)?;
            agent.require_enabled()?;
            if name.as_ref().is_some_and(|name| name != &agent.name)
                || platform
                    .as_ref()
                    .is_some_and(|platform| platform != &agent.platform)
            {
                return Err(
                    "That handle is already registered; use update_agent to change its profile"
                        .into(),
                );
            }
            return Ok(agent);
        }
        let name = name.unwrap_or_else(|| handle.clone());
        Self::validate_name(&name)?;
        let platform = platform.unwrap_or_default();
        platform.validate()?;
        let row = caller_storage::rows::<AgentRow>(
            "INSERT INTO relay_agents (handle, name, platform) VALUES (?, ?, ?) RETURNING id, wake, handle, name, platform, enabled",
            &(&handle, &name, serde_json::to_string(&platform)?),
        )?.pop().ok_or("The agent was not registered")?;
        Self::try_from(row)
    }

    pub(crate) fn get(id: u64) -> Result<Self> {
        let row = caller_storage::rows::<AgentRow>(
            "SELECT id, wake, handle, name, platform, enabled FROM relay_agents WHERE id = ?",
            &(id,),
        )?
        .pop()
        .ok_or_else(|| berm_lang::deny("No accessible Relay agent with that id"))?;
        Self::try_from(row)
    }

    pub fn list(after: Option<&str>, limit: Option<u64>) -> Result<Vec<Self>> {
        caller_storage::rows::<AgentRow>(
            "SELECT id, wake, handle, name, platform, enabled FROM relay_agents WHERE handle > ? ORDER BY handle LIMIT ?",
            &(after.unwrap_or(""), limit.unwrap_or(20).clamp(1, 20)),
        )?.into_iter().map(Self::try_from).collect()
    }

    pub fn update(
        handle: &str,
        name: Option<String>,
        platform: Option<Platform>,
        enabled: Option<bool>,
    ) -> Result<Self> {
        account::require_owner()?;
        let mut agent = Self::named(handle)?;
        let id = agent.id;
        if let Some(name) = name {
            Self::validate_name(&name)?;
            agent.name = name;
        }
        let reconnect = platform.is_some();
        if let Some(platform) = platform {
            platform.validate()?;
            agent.platform = platform;
        }
        if let Some(enabled) = enabled {
            agent.enabled = enabled;
        }
        caller_storage::rows::<()>(
            "UPDATE relay_agents SET name = ?, platform = ?, enabled = ? WHERE id = ?",
            &(
                &agent.name,
                serde_json::to_string(&agent.platform)?,
                agent.enabled,
                id,
            ),
        )?;
        if reconnect {
            Delivery::reset(&agent)?;
        }
        if reconnect || enabled.is_some() {
            Delivery::refresh(&agent, reconnect || enabled == Some(true))?;
            Delivery::schedule()?;
        }
        Ok(agent)
    }

    pub(crate) fn require_enabled(&self) -> Result<()> {
        if !self.enabled {
            return Err(berm_lang::deny("This Relay agent is disabled"));
        }
        Ok(())
    }

    pub fn named(handle: &str) -> Result<Self> {
        let handle = Self::handle(handle)?;
        let (id,) = caller_storage::rows::<(u64,)>(
            "SELECT id FROM relay_agents WHERE handle = ?",
            &(handle,),
        )?
        .pop()
        .ok_or_else(|| berm_lang::deny("No accessible Relay agent with that handle; check contacts or register your own unique handle"))?;
        Self::get(id)
    }

    pub fn connected(handle: Option<&str>) -> Result<Self> {
        let agent = if let Some(handle) = handle {
            Self::named(handle)?
        } else {
            let mut agents = Self::list(None, Some(2))?;
            if agents.is_empty() {
                return Err(berm_lang::deny("Register a handle with register"));
            }
            if agents.len() != 1 {
                return Err("Select an agent by handle".into());
            }
            agents.remove(0)
        };
        Ok(agent)
    }

    pub fn contacts(after: Option<&str>, limit: Option<u64>) -> Result<Contacts> {
        let limit = limit.unwrap_or(100).clamp(1, 100) as usize;
        let mut items = caller_storage::rows::<AgentRow>(
            "SELECT id, wake, handle, name, platform, enabled FROM relay_agents WHERE handle > ? ORDER BY handle LIMIT ?",
            &(after.unwrap_or(""), limit + 1),
        )?.into_iter().map(|row| Ok(Contact::from(Self::try_from(row)?))).collect::<Result<Vec<_>>>()?;
        let next = if items.len() > limit {
            items.truncate(limit);
            items.last().map(|agent| agent.handle.clone())
        } else {
            None
        };
        Ok(Contacts { items, next })
    }

    fn handle(input: &str) -> Result<String> {
        let handle = input
            .strip_prefix('@')
            .unwrap_or(input)
            .to_ascii_lowercase();
        if handle.is_empty()
            || handle.len() > 64
            || !handle
                .bytes()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_' || c == b'-')
            || !handle.bytes().any(|c| c.is_ascii_lowercase())
        {
            return Err("Use a handle of 1–64 letters, digits, underscores or hyphens, including at least one letter".into());
        }
        Ok(handle)
    }

    fn validate_name(name: &str) -> Result<()> {
        if name.trim().is_empty() || name.len() > 64 {
            return Err("An agent name must contain 1–64 bytes".into());
        }
        Ok(())
    }
}

type AgentRow = (u64, String, String, String, String, u8);

impl TryFrom<AgentRow> for Agent {
    type Error = berm_lang::Error;

    fn try_from((id, wake, handle, name, platform, enabled): AgentRow) -> Result<Self> {
        Ok(Self {
            id,
            wake,
            handle,
            name,
            platform: serde_json::from_str(&platform)?,
            enabled: enabled != 0,
        })
    }
}

#[derive(Schema)]
pub struct Contact {
    pub handle: String,
    pub name: String,
    pub platform: String,
    pub enabled: bool,
}

impl From<Agent> for Contact {
    fn from(agent: Agent) -> Self {
        Self {
            handle: agent.handle,
            name: agent.name,
            platform: String::from(match agent.platform {
                Platform::Inbox => "inbox",
                Platform::Grok(_) => "grok",
                Platform::OpenAiDot(_) => "open_ai_dot",
            }),
            enabled: agent.enabled,
        }
    }
}

#[derive(Schema)]
pub struct Contacts {
    pub items: Vec<Contact>,
    pub next: Option<String>,
}
