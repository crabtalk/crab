//! ID tokens this program issues, signed by the host with this program's key,
//! for a manifest that declares `berm.token.sign`. This program never holds
//! the key.

use crate::abi::{self, host, host::CallError, wire};
use alloc::{format, string::String};

/// Sign `claims` as a compact JWT and answer it.
///
/// ```ignore
/// let token = berm_lang::token::sign(&serde_json::json!({
///     "sub": "user-1",
///     "aud": "https://app.example.com",
///     "exp": berm_lang::now() / 1000 + 600,
/// }))?;
/// ```
///
/// `claims` is a JSON object carrying a string `sub`, an `aud` that is a
/// string or an array of strings, and a numeric `exp`; anything else is
/// [`CallError::Refused`]. The host sets `iss` to [`issuer`] and `iat` to its
/// clock in seconds, over whatever `claims` held.
pub fn sign(claims: &impl serde::Serialize) -> Result<String, CallError> {
    let claims = serde_json::to_vec(claims)
        .map_err(|error| CallError::Failed(format!("unserializable claims: {error}")))?;
    one(&host::call(
        abi::HOST_TOKEN_SIGN,
        &wire::request(&[&claims]),
    )?)
}

/// This program's issuer URL, the `iss` of every token [`sign`] answers.
pub fn issuer() -> Result<String, CallError> {
    one(&host::call(abi::HOST_TOKEN_ISSUER, &[])?)
}

/// The one UTF-8 field a reply carries.
fn one(reply: &[u8]) -> Result<String, CallError> {
    match wire::fields(reply).as_deref() {
        Some([field]) => String::from_utf8(field.to_vec())
            .map_err(|_| CallError::Failed(String::from("host answered text that is not UTF-8"))),
        _ => Err(CallError::Failed(String::from(
            "host framed a reply this guest cannot read",
        ))),
    }
}
