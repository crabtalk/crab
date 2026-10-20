//! `berm.token.sign` and `berm.token.issuer`, served by the daemon's
//! [`Env::Issuer`].

use crate::{
    env::{Env, Issuer},
    host::{self, Inner},
    key::base64url,
};
use berm::{
    Callsite, Syscall,
    syscall::token::{self, Claims},
};
use serde_json::json;
use std::sync::Weak;

pub(crate) fn syscalls<E: Env>(host: &Weak<Inner<E>>) -> Vec<Syscall> {
    let signing = host.clone();
    let sign = move |at: Callsite, mut claims: Claims| {
        let host = host::alive(&signing);
        async move {
            let host = host?;
            let issuer = host.issuer.issuer(&at.program).await?;
            claims.insert("iss".to_owned(), issuer.into());
            claims.insert("iat".to_owned(), ((host.now)() / 1000).into());
            let kid = host.issuer.kid(&at.program).await?;
            let header = json!({"alg": "EdDSA", "typ": "JWT", "kid": kid});
            let input = format!(
                "{}.{}",
                base64url(header.to_string()),
                base64url(serde_json::to_vec(&claims)?)
            );
            let signature = host.issuer.sign(&at.program, input.as_bytes()).await?;
            Ok(format!("{input}.{}", base64url(signature)))
        }
    };
    let naming = host.clone();
    let issuer = move |at: Callsite| {
        let host = host::alive(&naming);
        async move { host?.issuer.issuer(&at.program).await }
    };
    token::programs(sign, issuer)
}
