use crate::{StripeError, store};
use alloc::string::String;
use berm_lang::{Result, User};

/// The end user the call carries.
pub fn user() -> Result<User> {
    Ok(berm_lang::user()?.ok_or(StripeError::NoUser)?)
}

/// `user`'s Stripe customer, once they have checked out.
pub fn customer(user: &User) -> Result<Option<String>> {
    let found = store::sql::<(String,)>(
        "SELECT customer FROM customers WHERE issuer = ? AND subject = ?",
        &(&user.issuer, &user.subject),
    )?;
    Ok(found.into_iter().next().map(|(customer,)| customer))
}

/// `fields` as `application/x-www-form-urlencoded`.
pub fn form(fields: &[(&str, &str)]) -> String {
    let mut out = String::new();
    for (name, value) in fields {
        if !out.is_empty() {
            out.push('&');
        }
        out.push_str(&encode(name));
        out.push('=');
        out.push_str(&encode(value));
    }
    out
}

/// `text` percent-encoded, leaving only unreserved characters.
pub fn encode(text: &str) -> String {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut out = String::new();
    for byte in text.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                out.push(byte as char)
            }
            _ => {
                out.push('%');
                out.push(HEX[(byte >> 4) as usize] as char);
                out.push(HEX[(byte & 15) as usize] as char);
            }
        }
    }
    out
}
