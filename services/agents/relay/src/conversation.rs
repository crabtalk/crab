use crate::{account::Account, agent::Agent, delivery::Delivery};
use alloc::{string::String, vec::Vec};
use berm_lang::{Result, Schema};

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
        let owner = Account::require_owner()?.id;
        let members = members
            .iter()
            .map(|handle| Ok(Agent::named(handle)?.id))
            .collect::<Result<Vec<_>>>()?;
        Self::create(owner, members, budget)
    }

    pub(crate) fn direct(from: u64, to: u64) -> Result<Self> {
        let owner = Account::current()?.id;
        let found = berm_lang::rows::<(u64,)>(
            "SELECT c.id FROM relay_conversations c WHERE c.owner = ? AND (SELECT COUNT(*) FROM relay_members m WHERE m.conversation = c.id) = 2 AND EXISTS (SELECT 1 FROM relay_members m WHERE m.conversation = c.id AND m.agent = ?) AND EXISTS (SELECT 1 FROM relay_members m WHERE m.conversation = c.id AND m.agent = ?) ORDER BY c.id DESC LIMIT 1",
            &(&owner, from, to),
        )?.pop();
        match found {
            Some((id,)) => Self::get(id),
            None => Self::create(owner, alloc::vec![from, to], None),
        }
    }

    fn create(
        owner: alloc::string::String,
        members: Vec<u64>,
        budget: Option<u64>,
    ) -> Result<Self> {
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
        Self::get(id)
    }

    pub fn get(id: u64) -> Result<Self> {
        let owner = Account::current()?.id;
        let (paused, budget, sent) = berm_lang::rows::<(u8, u64, u64)>(
            "SELECT paused, budget, sent FROM relay_conversations WHERE id = ? AND owner = ?",
            &(id, owner),
        )?
        .pop()
        .ok_or_else(|| berm_lang::deny("No accessible conversation with that id"))?;
        let members = berm_lang::rows::<(String,)>(
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
        Account::require_owner()?;
        let mut conversation = Self::get(id)?;
        berm_lang::rows::<()>(
            "UPDATE relay_conversations SET paused = ? WHERE id = ?",
            &(paused, id),
        )?;
        conversation.paused = paused;
        if !paused {
            Delivery::schedule()?;
        }
        Ok(conversation)
    }

    pub(crate) fn require_member(&self, id: u64) -> Result<()> {
        if berm_lang::rows::<(u8,)>(
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
