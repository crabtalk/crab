use crate::{
    account::Account,
    delivery::Delivery,
    platform::{DotConnection, Platform},
};
use alloc::{string::String, vec::Vec};
use berm_lang::{Result, Schema, key, serde_json};

pub struct Agent {
    pub(crate) id: u64,
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
        let owner = Account::current()?.id;
        let handle = Self::handle(&handle)?;
        if let Some((id,)) = berm_lang::rows::<(u64,)>(
            "SELECT id FROM relay_agents WHERE owner = ? AND handle = ?",
            &(&owner, &handle),
        )?
        .pop()
        {
            let agent = Self::get(id)?;
            agent.require_key()?;
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
        let platform = platform.unwrap_or(Platform::OpenAiDot(DotConnection {}));
        platform.validate()?;
        let (id,) = berm_lang::rows::<(u64,)>(
            "INSERT INTO relay_agents (owner, handle, name, platform) VALUES (?, ?, ?, ?) RETURNING id",
            &(&owner, &handle, &name, serde_json::to_string(&platform)?),
        )?.pop().ok_or("The agent was not registered")?;
        let agent = Self {
            id,
            handle,
            name,
            platform,
            enabled: true,
        };
        if let Some(key) = key::id()? {
            agent.allow(&key)?;
        }
        Ok(agent)
    }

    pub(crate) fn get(id: u64) -> Result<Self> {
        Self::load(id, Some(&Account::current()?.id))
    }

    pub(crate) fn load(id: u64, owner: Option<&str>) -> Result<Self> {
        let row = berm_lang::rows::<(u64, String, String, String, u8)>(
            "SELECT id, handle, name, platform, enabled FROM relay_agents WHERE id = ? AND (? IS NULL OR owner = ?)",
            &(id, owner, owner),
        )?
        .pop()
        .ok_or_else(|| berm_lang::deny("No accessible Relay agent with that id"))?;
        Self::try_from(row)
    }

    pub fn list(after: Option<&str>, limit: Option<u64>) -> Result<Vec<Self>> {
        let owner = Account::current()?.id;
        let key = key::id()?;
        berm_lang::rows::<(u64, String, String, String, u8)>(
            "SELECT id, handle, name, platform, enabled FROM relay_agents WHERE owner = ? AND handle > ? AND (? IS NULL OR EXISTS (SELECT 1 FROM relay_agent_keys WHERE agent = relay_agents.id AND key_id = ?)) ORDER BY handle LIMIT ?",
            &(owner, after.unwrap_or(""), &key, &key, limit.unwrap_or(20).clamp(1, 20)),
        )?.into_iter().map(Self::try_from).collect()
    }

    pub fn update(
        handle: &str,
        name: Option<String>,
        platform: Option<Platform>,
        enabled: Option<bool>,
    ) -> Result<Self> {
        Account::require_owner()?;
        let mut agent = Self::named(handle)?;
        let id = agent.id;
        if let Some(name) = name {
            Self::validate_name(&name)?;
            agent.name = name;
        }
        let mut connection_changed = false;
        if let Some(platform) = platform {
            platform.validate()?;
            connection_changed = agent.platform != platform;
            agent.platform = platform;
        }
        if let Some(enabled) = enabled {
            agent.enabled = enabled;
        }
        berm_lang::rows::<()>(
            "UPDATE relay_agents SET name = ?, platform = ?, enabled = ? WHERE id = ?",
            &(
                &agent.name,
                serde_json::to_string(&agent.platform)?,
                agent.enabled,
                id,
            ),
        )?;
        if connection_changed {
            berm_lang::rows::<()>(
                "UPDATE relay_deliveries SET state = 'cancelled', reason = 'connection_changed' WHERE state = 'pending' AND message IN (SELECT id FROM relay_messages WHERE recipient = ?)",
                &(id,),
            )?;
        }
        if connection_changed || enabled == Some(true) {
            Delivery::schedule()?;
        }
        Ok(agent)
    }

    pub fn keys(handle: &str) -> Result<Vec<String>> {
        Account::require_owner()?;
        let id = Self::named(handle)?.id;
        Ok(berm_lang::rows::<(String,)>(
            "SELECT key_id FROM relay_agent_keys WHERE agent = ? ORDER BY key_id",
            &(id,),
        )?
        .into_iter()
        .map(|(key,)| key)
        .collect())
    }

    pub fn grant_key(handle: &str, key: String) -> Result<()> {
        Account::require_owner()?;
        Self::validate_key(&key)?;
        Self::named(handle)?.allow(&key)
    }

    fn allow(&self, key: &str) -> Result<()> {
        berm_lang::rows::<()>(
            "INSERT INTO relay_agent_keys (agent, key_id) VALUES (?, ?) ON CONFLICT DO NOTHING",
            &(self.id, key),
        )?;
        Ok(())
    }

    pub fn revoke_key(handle: &str, key: String) -> Result<()> {
        Account::require_owner()?;
        let id = Self::named(handle)?.id;
        berm_lang::rows::<()>(
            "DELETE FROM relay_agent_keys WHERE agent = ? AND key_id = ?",
            &(id, key),
        )?;
        Ok(())
    }

    pub(crate) fn require_key(&self) -> Result<()> {
        if let Some(key) = key::id()?
            && berm_lang::rows::<(u8,)>(
                "SELECT 1 FROM relay_agent_keys WHERE agent = ? AND key_id = ?",
                &(self.id, key),
            )?
            .is_empty()
        {
            return Err(berm_lang::deny(
                "This connection cannot use this agent. Approve agent access in Settings → Connections",
            ));
        }
        Ok(())
    }

    pub(crate) fn require_enabled(&self) -> Result<()> {
        if !self.enabled {
            return Err(berm_lang::deny("This Relay agent is disabled"));
        }
        Ok(())
    }

    pub fn named(handle: &str) -> Result<Self> {
        let owner = Account::current()?.id;
        let handle = Self::handle(handle)?;
        let (id,) = berm_lang::rows::<(u64,)>(
            "SELECT id FROM relay_agents WHERE owner = ? AND handle = ?",
            &(owner, handle),
        )?
        .pop()
        .ok_or_else(|| berm_lang::deny("No accessible Relay agent with that handle; check contacts or register your own unique handle"))?;
        Self::get(id)
    }

    pub(crate) fn recipient(message: u64) -> Result<Self> {
        let (id,) = berm_lang::rows::<(u64,)>(
            "SELECT recipient FROM relay_messages WHERE id = ?",
            &(message,),
        )?
        .pop()
        .ok_or("No such message")?;
        Self::load(id, None)
    }

    pub fn connected(handle: Option<&str>) -> Result<Self> {
        let agent = if let Some(handle) = handle {
            Self::named(handle)?
        } else {
            let mut agents = Self::list(None, Some(2))?;
            if agents.is_empty() {
                return Err(berm_lang::deny(
                    "Register your own handle with register, or ask the account owner for access to an existing handle in Settings → Connections",
                ));
            }
            if agents.len() != 1 {
                return Err("Select your registered handle".into());
            }
            agents.remove(0)
        };
        agent.require_key()?;
        Ok(agent)
    }

    pub fn contacts(after: Option<&str>, limit: Option<u64>) -> Result<Contacts> {
        let owner = Account::current()?.id;
        let limit = limit.unwrap_or(100).clamp(1, 100) as usize;
        let mut items = berm_lang::rows::<(u64, String, String, String, u8)>(
            "SELECT id, handle, name, platform, enabled FROM relay_agents WHERE owner = ? AND handle > ? ORDER BY handle LIMIT ?",
            &(owner, after.unwrap_or(""), limit + 1),
        )?.into_iter().map(|row| Ok(Contact::from(Self::try_from(row)?))).collect::<Result<Vec<_>>>()?;
        let next = if items.len() > limit {
            items.truncate(limit);
            items.last().map(|agent| agent.handle.clone())
        } else {
            None
        };
        Ok(Contacts { items, next })
    }

    pub fn access(
        key_id: Option<&str>,
        after: Option<&str>,
        limit: Option<u64>,
    ) -> Result<AccessPage> {
        let owner = Account::require_owner()?.id;
        let limit = limit.unwrap_or(100).clamp(1, 100) as usize;
        let mut items = berm_lang::rows::<(u64, String, String, String, u8, u8)>(
            "SELECT a.id, a.handle, a.name, a.platform, a.enabled, EXISTS (SELECT 1 FROM relay_agent_keys k WHERE k.agent = a.id AND k.key_id = ?) FROM relay_agents a WHERE a.owner = ? AND a.handle > ? ORDER BY a.handle LIMIT ?",
            &(key_id, owner, after.unwrap_or(""), limit + 1),
        )?.into_iter().map(|(id, handle, name, platform, enabled, granted)| Ok(AgentAccess {
            agent: Contact::from(Self::try_from((id, handle, name, platform, enabled))?),
            granted: granted != 0,
        })).collect::<Result<Vec<_>>>()?;
        let next = if items.len() > limit {
            items.truncate(limit);
            items.last().map(|entry| entry.agent.handle.clone())
        } else {
            None
        };
        Ok(AccessPage { items, next })
    }

    pub fn connect(key_id: String, agents: Vec<String>) -> Result<Vec<Contact>> {
        Account::require_owner()?;
        Self::validate_key(&key_id)?;
        if agents.len() > 100 {
            return Err("Choose up to 100 agents for an authenticated connection".into());
        }
        let mut selected = Vec::new();
        for handle in agents {
            let agent = Self::named(&handle)?;
            agent.require_enabled()?;
            if selected.contains(&agent.id) {
                return Err("Choose each agent once".into());
            }
            selected.push(agent.id);
        }
        let owner = Account::current()?.id;
        berm_lang::rows::<()>(
            "DELETE FROM relay_agent_keys WHERE key_id = ? AND agent IN (SELECT id FROM relay_agents WHERE owner = ?) AND agent NOT IN (SELECT value FROM json_each(?))",
            &(&key_id, owner, serde_json::to_string(&selected)?),
        )?;
        let mut connected = Vec::new();
        for id in selected {
            let agent = Self::get(id)?;
            agent.allow(&key_id)?;
            connected.push(Contact::from(agent));
        }
        Ok(connected)
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

    fn validate_key(key: &str) -> Result<()> {
        if key.is_empty() || key.len() > 128 || key.chars().any(char::is_control) {
            return Err("Use an API key ID of 1–128 bytes".into());
        }
        Ok(())
    }

    fn validate_name(name: &str) -> Result<()> {
        if name.trim().is_empty() || name.len() > 64 {
            return Err("An agent name must contain 1–64 bytes".into());
        }
        Ok(())
    }
}

impl TryFrom<(u64, String, String, String, u8)> for Agent {
    type Error = berm_lang::Error;

    fn try_from(
        (id, handle, name, platform, enabled): (u64, String, String, String, u8),
    ) -> Result<Self> {
        Ok(Self {
            id,
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
                Platform::Grok(_) => "grok",
                Platform::OpenAiDot(_) => "open_ai_dot",
            }),
            enabled: agent.enabled,
        }
    }
}

#[derive(Schema)]
pub struct AgentAccess {
    pub agent: Contact,
    pub granted: bool,
}

#[derive(Schema)]
pub struct Contacts {
    pub items: Vec<Contact>,
    pub next: Option<String>,
}

#[derive(Schema)]
pub struct AccessPage {
    pub items: Vec<AgentAccess>,
    pub next: Option<String>,
}
