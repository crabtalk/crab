use crate::{account::Account, delivery::Delivery, platform::Platform};
use alloc::{string::String, vec::Vec};
use berm_lang::{Result, Schema, key, serde_json};

#[derive(Schema)]
pub struct Agent {
    pub id: u64,
    pub name: String,
    pub platform: Platform,
    pub enabled: bool,
}

impl Agent {
    pub fn register(name: String, platform: Platform) -> Result<Self> {
        let owner = Account::require_owner()?.id;
        Self::validate_name(&name)?;
        platform.validate()?;
        let (id,) = berm_lang::rows::<(u64,)>(
            "INSERT INTO relay_agents (owner, name, platform) VALUES (?, ?, ?) RETURNING id",
            &(&owner, &name, serde_json::to_string(&platform)?),
        )?
        .pop()
        .ok_or("The agent was not registered")?;
        Ok(Self {
            id,
            name,
            platform,
            enabled: true,
        })
    }

    pub fn get(id: u64) -> Result<Self> {
        Self::load(id, Some(&Account::current()?.id))
    }

    pub(crate) fn load(id: u64, owner: Option<&str>) -> Result<Self> {
        let row = berm_lang::rows::<(u64, String, String, u8)>(
            "SELECT id, name, platform, enabled FROM relay_agents WHERE id = ? AND (? IS NULL OR owner = ?)",
            &(id, owner, owner),
        )?
        .pop()
        .ok_or_else(|| berm_lang::deny("No accessible Relay agent with that id"))?;
        Self::try_from(row)
    }

    pub fn list(after: u64, limit: Option<u64>) -> Result<Vec<Self>> {
        let owner = Account::current()?.id;
        let key = key::id()?;
        berm_lang::rows::<(u64, String, String, u8)>(
            "SELECT id, name, platform, enabled FROM relay_agents WHERE owner = ? AND id > ? AND (? IS NULL OR EXISTS (SELECT 1 FROM relay_agent_keys WHERE agent = relay_agents.id AND key_id = ?)) ORDER BY id LIMIT ?",
            &(owner, after, &key, &key, limit.unwrap_or(20).clamp(1, 20)),
        )?.into_iter().map(Self::try_from).collect()
    }

    pub fn update(
        id: u64,
        name: Option<String>,
        platform: Option<Platform>,
        enabled: Option<bool>,
    ) -> Result<Self> {
        let mut agent = Self::load(id, Some(&Account::require_owner()?.id))?;
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

    pub fn keys(id: u64) -> Result<Vec<String>> {
        Self::load(id, Some(&Account::require_owner()?.id))?;
        Ok(berm_lang::rows::<(String,)>(
            "SELECT key_id FROM relay_agent_keys WHERE agent = ? ORDER BY key_id",
            &(id,),
        )?
        .into_iter()
        .map(|(key,)| key)
        .collect())
    }

    pub fn grant_key(id: u64, key: String) -> Result<()> {
        Self::load(id, Some(&Account::require_owner()?.id))?;
        if key.is_empty() || key.len() > 128 || key.chars().any(char::is_control) {
            return Err("Use an API key ID of 1–128 bytes".into());
        }
        berm_lang::rows::<()>(
            "INSERT INTO relay_agent_keys (agent, key_id) VALUES (?, ?) ON CONFLICT DO NOTHING",
            &(id, key),
        )?;
        Ok(())
    }

    pub fn revoke_key(id: u64, key: String) -> Result<()> {
        Self::load(id, Some(&Account::require_owner()?.id))?;
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
                "This API key has no grant for the Relay agent",
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

    fn validate_name(name: &str) -> Result<()> {
        if name.trim().is_empty() || name.len() > 64 {
            return Err("An agent name must contain 1–64 bytes".into());
        }
        Ok(())
    }
}

impl TryFrom<(u64, String, String, u8)> for Agent {
    type Error = berm_lang::Error;

    fn try_from((id, name, platform, enabled): (u64, String, String, u8)) -> Result<Self> {
        Ok(Self {
            id,
            name,
            platform: serde_json::from_str(&platform)?,
            enabled: enabled != 0,
        })
    }
}
