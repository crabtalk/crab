use crate::{account, agent::Agent, delivery::Delivery};
use alloc::{string::String, vec::Vec};
use berm_lang::{Result, Schema, caller_storage};

#[derive(Schema)]
pub struct Conversation {
    pub id: u64,
    pub members: Vec<String>,
    pub paused: bool,
    pub budget: u64,
    pub sent: u64,
}

impl Conversation {
    pub fn open(members: Vec<String>, budget: Option<u64>) -> Result<Self> {
        account::require_owner()?;
        let members = members
            .iter()
            .map(|handle| Ok(Agent::named(handle)?.id))
            .collect::<Result<Vec<_>>>()?;
        Self::create(members, budget)
    }

    pub(crate) fn direct(from: u64, to: u64) -> Result<Self> {
        let found = caller_storage::rows::<(u64,)>(
            "SELECT c.id FROM relay_conversations c WHERE (SELECT COUNT(*) FROM relay_members m WHERE m.conversation = c.id) = 2 AND EXISTS (SELECT 1 FROM relay_members m WHERE m.conversation = c.id AND m.agent = ?) AND EXISTS (SELECT 1 FROM relay_members m WHERE m.conversation = c.id AND m.agent = ?) ORDER BY c.id DESC LIMIT 1",
            &(from, to),
        )?.pop();
        match found {
            Some((id,)) => Self::get(id),
            None => Self::create(alloc::vec![from, to], None),
        }
    }

    fn create(members: Vec<u64>, budget: Option<u64>) -> Result<Self> {
        if !(2..=16).contains(&members.len()) {
            return Err("A conversation needs 2–16 agents".into());
        }
        for (at, id) in members.iter().enumerate() {
            if members[..at].contains(id) {
                return Err("List each conversation member once".into());
            }
            Agent::get(*id)?.require_enabled()?;
        }
        let budget = budget.unwrap_or(100);
        if !(1..=1000).contains(&budget) {
            return Err("A conversation budget must be 1–1000 messages".into());
        }
        let (id,) = caller_storage::rows::<(u64,)>(
            "INSERT INTO relay_conversations (budget) VALUES (?) RETURNING id",
            &(budget,),
        )?
        .pop()
        .ok_or("The conversation was not created")?;
        for member in &members {
            caller_storage::rows::<()>(
                "INSERT INTO relay_members (conversation, agent) VALUES (?, ?)",
                &(id, member),
            )?;
        }
        Self::get(id)
    }

    pub fn get(id: u64) -> Result<Self> {
        let (paused, budget, sent) = caller_storage::rows::<(u8, u64, u64)>(
            "SELECT paused, budget, sent FROM relay_conversations WHERE id = ?",
            &(id,),
        )?
        .pop()
        .ok_or_else(|| berm_lang::deny("No accessible conversation with that id"))?;
        let members = caller_storage::rows::<(String,)>(
            "SELECT a.handle FROM relay_members m JOIN relay_agents a ON a.id = m.agent WHERE m.conversation = ? ORDER BY a.handle",
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
        account::require_owner()?;
        let mut conversation = Self::get(id)?;
        caller_storage::rows::<()>(
            "UPDATE relay_conversations SET paused = ? WHERE id = ?",
            &(paused, id),
        )?;
        conversation.paused = paused;
        for (recipient,) in caller_storage::rows::<(u64,)>(
            "SELECT DISTINCT m.recipient FROM relay_messages m
             JOIN relay_agents a ON a.id = m.recipient
             WHERE m.conversation = ? AND m.id > a.read_at",
            &(id,),
        )? {
            Delivery::refresh(&Agent::get(recipient)?, !paused)?;
        }
        Delivery::schedule()?;
        Ok(conversation)
    }

    pub(crate) fn require_member(&self, id: u64) -> Result<()> {
        if caller_storage::rows::<(u8,)>(
            "SELECT 1 FROM relay_members WHERE conversation = ? AND agent = ?",
            &(self.id, id),
        )?
        .is_empty()
        {
            return Err(berm_lang::deny(
                "The agent is not a member of this conversation",
            ));
        }
        Agent::get(id)?.require_enabled()
    }
}
