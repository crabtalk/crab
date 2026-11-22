use alloc::string::String;
use berm_lang::{Caller, Result, caller, caller_storage, deny, random, user};

const CLAIM_ACCOUNT: &str = "claim_account";

pub(crate) struct Account {
    pub member: String,
    pub claim_id: Option<String>,
}

impl Account {
    pub fn calling() -> Result<Self> {
        let Some(Caller::Outside(member)) = caller()? else {
            return Err(deny("Use an account session or API key"));
        };
        if user()?.is_some() {
            return Err(deny("Use an account session or API key"));
        }
        Ok(Self {
            member,
            claim_id: caller_storage::get(CLAIM_ACCOUNT)?,
        })
    }

    // The guest sees the API key's creator. Caller storage belongs to the key's account.
    pub fn claim_id(&mut self) -> Result<&str> {
        match &mut self.claim_id {
            Some(id) => Ok(id),
            slot @ None => {
                let mut bytes = [0; 16];
                random(&mut bytes);
                let id = hex::encode(bytes);
                caller_storage::set(CLAIM_ACCOUNT, &id)?;
                Ok(slot.insert(id))
            }
        }
    }
}
