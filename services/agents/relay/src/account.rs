use berm_lang::{Caller, Result, key};

pub(crate) fn require_owner() -> Result<()> {
    if key::id()?.is_some() {
        return Err(berm_lang::deny(
            "Use an owner session to manage Relay agents and conversations",
        ));
    }
    if !matches!(berm_lang::caller()?, Some(Caller::Outside(_))) {
        return Err(berm_lang::deny("Sign in to use Relay"));
    }
    Ok(())
}
