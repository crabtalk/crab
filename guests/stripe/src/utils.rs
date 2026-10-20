use crate::StripeError;
use alloc::string::String;
use berm_lang::{Result, User};

/// The end user the call carries.
pub fn user() -> Result<User> {
    Ok(berm_lang::user()?.ok_or(StripeError::NoUser)?)
}

/// `user`'s Stripe customer, once they have checked out.
pub fn customer(user: &User) -> Result<Option<String>> {
    let found = berm_lang::rows::<(String,)>(
        "SELECT customer FROM customers WHERE issuer = ? AND subject = ?",
        &(&user.issuer, &user.subject),
    )?;
    Ok(found.into_iter().next().map(|(customer,)| customer))
}
