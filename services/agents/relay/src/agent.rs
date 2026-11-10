use crate::{account::Account, platform::Platform};
use alloc::{string::String, vec::Vec};
use berm_lang::{Result, Schema, serde_json};

#[derive(Schema)]
pub struct Agent {
    pub id: u64,
    pub name: String,
    pub platform: Platform,
    pub enabled: bool,
}

impl Agent {
    pub fn register(name: String, platform: Platform) -> Result<Self> {
        Self::validate_name(&name)?;
        platform.validate()?;
        let owner = Account::current()?.id;
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
        let owner = Account::current()?.id;
        let row = berm_lang::rows::<(u64, String, String, u8)>(
            "SELECT id, name, platform, enabled FROM relay_agents WHERE id = ? AND owner = ?",
            &(id, owner),
        )?
        .pop()
        .ok_or_else(|| berm_lang::deny("No accessible Relay agent with that id"))?;
        Self::try_from(row)
    }

    pub fn list(after: u64, limit: Option<u64>) -> Result<Vec<Self>> {
        let owner = Account::current()?.id;
        berm_lang::rows::<(u64, String, String, u8)>(
            "SELECT id, name, platform, enabled FROM relay_agents WHERE owner = ? AND id > ? ORDER BY id LIMIT ?",
            &(owner, after, limit.unwrap_or(20).clamp(1, 20)),
        )?.into_iter().map(Self::try_from).collect()
    }

    pub fn update(
        id: u64,
        name: Option<String>,
        platform: Option<Platform>,
        enabled: Option<bool>,
    ) -> Result<Self> {
        let mut agent = Self::get(id)?;
        if let Some(name) = name {
            Self::validate_name(&name)?;
            agent.name = name;
        }
        if let Some(platform) = platform {
            platform.validate()?;
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
        Ok(agent)
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
