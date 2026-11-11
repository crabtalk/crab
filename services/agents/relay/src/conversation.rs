use crate::{account::Account, agent::Agent};
use alloc::vec::Vec;
use berm_lang::{Result, Schema, key};

#[derive(Schema)]
pub struct Conversation {
    pub id: u64,
    pub members: Vec<u64>,
    pub paused: bool,
    pub budget: u64,
    pub sent: u64,
}

impl Conversation {
    pub fn open(members: Vec<u64>, budget: Option<u64>) -> Result<Self> {
        let owner = Account::require_owner()?.id;
        if !(2..=16).contains(&members.len()) {
            return Err("A conversation needs 2–16 agents".into());
        }
        for (at, id) in members.iter().enumerate() {
            if members[..at].contains(id) {
                return Err("List each conversation member once".into());
            }
            Agent::load(*id, Some(&owner))?.require_enabled()?;
        }
        let budget = budget.unwrap_or(100);
        if !(1..=1000).contains(&budget) {
            return Err("A conversation budget must be 1–1000 messages".into());
        }
        let (id,) = berm_lang::rows::<(u64,)>(
            "INSERT INTO relay_conversations (owner, budget) VALUES (?, ?) RETURNING id",
            &(owner, budget),
        )?
        .pop()
        .ok_or("The conversation was not created")?;
        for member in &members {
            berm_lang::rows::<()>(
                "INSERT INTO relay_members (conversation, agent) VALUES (?, ?)",
                &(id, member),
            )?;
        }
        Ok(Self {
            id,
            members,
            paused: false,
            budget,
            sent: 0,
        })
    }

    pub fn get(id: u64) -> Result<Self> {
        let owner = Account::current()?.id;
        let key = key::id()?;
        let (paused, budget, sent, permitted) = berm_lang::rows::<(u8, u64, u64, u8)>(
            "SELECT paused, budget, sent, (? IS NULL OR EXISTS (SELECT 1 FROM relay_members m JOIN relay_agent_keys k ON k.agent = m.agent WHERE m.conversation = c.id AND k.key_id = ?)) FROM relay_conversations c WHERE id = ? AND owner = ?",
            &(&key, &key, id, owner),
        )?
        .pop()
        .ok_or_else(|| berm_lang::deny("No accessible conversation with that id"))?;
        if permitted == 0 {
            return Err(berm_lang::deny(
                "This API key has no agent in the conversation",
            ));
        }
        let members = berm_lang::rows::<(u64,)>(
            "SELECT agent FROM relay_members WHERE conversation = ? ORDER BY agent",
            &(id,),
        )?
        .into_iter()
        .map(|(id,)| id)
        .collect();
        Ok(Self {
            id,
            members,
            paused: paused != 0,
            budget,
            sent,
        })
    }

    pub fn pause(id: u64, paused: bool) -> Result<Self> {
        Account::require_owner()?;
        let mut conversation = Self::get(id)?;
        berm_lang::rows::<()>(
            "UPDATE relay_conversations SET paused = ? WHERE id = ?",
            &(paused, id),
        )?;
        conversation.paused = paused;
        Ok(conversation)
    }

    pub(crate) fn require_member(&self, id: u64) -> Result<()> {
        if !self.members.contains(&id) {
            return Err(berm_lang::deny(
                "The agent is not a member of this conversation",
            ));
        }
        Agent::get(id)?.require_enabled()
    }
}
