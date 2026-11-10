use alloc::string::String;
use berm_lang::{Caller, Result, serde_json};

pub(crate) struct Account {
    pub(crate) id: String,
}

impl Account {
    pub(crate) fn current() -> Result<Self> {
        let id = if let Some(user) = berm_lang::user()? {
            serde_json::to_string(&("user", user.issuer, user.subject))?
        } else if let Some(Caller::Outside(account)) = berm_lang::caller()? {
            serde_json::to_string(&("account", account))?
        } else {
            return Err(berm_lang::deny("Sign in to use Relay"));
        };
        Ok(Self { id })
    }
}
