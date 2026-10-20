//! `berm.token.sign` and `berm.token.issuer` — ID tokens signed with the
//! asking program's key, which the guest never holds.
//!
//! berm reads the request, refuses a program whose manifest does not declare
//! `berm.token.sign`, and refuses claims that are not a JSON object carrying a
//! string `sub`, an `aud` that is a string or an array of strings, and a
//! numeric `exp`. Setting `iss` and `iat`, and signing, are the host's, and
//! arrive as the closures below.

use crate::{Callsite, Refused, Syscall, abi, wire};
use anyhow::Result;
use serde_json::{Map, Value};
use std::{future::Future, sync::Arc};

/// A token's claims, as the guest asked for them.
pub type Claims = Map<String, Value>;

/// Serve both doors: `sign` is handed the callsite and checked claims and
/// answers the compact JWT, and `issuer` answers the asking program's issuer
/// URL.
pub fn programs<S, Signed, I, Named>(sign: S, issuer: I) -> Vec<Syscall>
where
    S: Fn(Callsite, Claims) -> Signed + Send + Sync + 'static,
    Signed: Future<Output = Result<String>> + Send + 'static,
    I: Fn(Callsite) -> Named + Send + Sync + 'static,
    Named: Future<Output = Result<String>> + Send + 'static,
{
    let (sign, issuer) = (Arc::new(sign), Arc::new(issuer));
    vec![
        Syscall {
            name: abi::TOKEN_SIGN.to_owned(),
            call: Arc::new(move |at: Callsite, request: Vec<u8>| {
                let sign = sign.clone();
                Box::pin(async move {
                    declared(&at)?;
                    let fields = wire::fields(&request)?;
                    let claims = claims(wire::text(&fields, 0, "claims")?)?;
                    let token = sign(at, claims).await?;
                    Ok(wire::frame(&[token.as_bytes()]))
                })
            }),
        },
        Syscall {
            name: abi::TOKEN_ISSUER.to_owned(),
            call: Arc::new(move |at: Callsite, _: Vec<u8>| {
                let issuer = issuer.clone();
                Box::pin(async move {
                    declared(&at)?;
                    let issuer = issuer(at).await?;
                    Ok(wire::frame(&[issuer.as_bytes()]))
                })
            }),
        },
    ]
}

/// Refuse a program whose manifest does not declare `berm.token.sign`.
fn declared(at: &Callsite) -> Result<()> {
    if !at.deps.syscalls.contains_key(abi::TOKEN_SIGN) {
        return Err(Refused(format!(
            "{} does not declare {}",
            at.program,
            abi::TOKEN_SIGN
        ))
        .into());
    }
    Ok(())
}

/// `text` as claims a token may carry.
fn claims(text: &str) -> Result<Claims> {
    let refused = |why: &str| Err(Refused(format!("the claims {why}")).into());
    let Ok(Value::Object(claims)) = serde_json::from_str(text) else {
        return refused("are not a JSON object");
    };
    if !claims.get("sub").is_some_and(Value::is_string) {
        return refused("carry no string `sub`");
    }
    let audience = match claims.get("aud") {
        Some(Value::String(_)) => true,
        Some(Value::Array(audiences)) => {
            !audiences.is_empty() && audiences.iter().all(Value::is_string)
        }
        _ => false,
    };
    if !audience {
        return refused("carry no `aud` that is a string or an array of strings");
    }
    if !claims.get("exp").is_some_and(Value::is_number) {
        return refused("carry no numeric `exp`");
    }
    Ok(claims)
}
