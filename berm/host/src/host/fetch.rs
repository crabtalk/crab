//! `berm.http.fetch` and `berm.http.proxy`, sent through the daemon's
//! [`Env::Fetch`].

use crate::{
    env::{Env, Secrets},
    host::{self, Inner},
};
use anyhow::Context;
use berm::{
    Callsite, Syscall,
    syscall::http::{self, Request},
};
use std::sync::Weak;
use tower::ServiceExt;

/// `berm.http.proxy` when `proxied`, `berm.http.fetch` otherwise, sent
/// through `fetch` with `host`'s secrets, each request given up at its call's
/// deadline.
pub(crate) fn syscall<E: Env>(host: &Weak<Inner<E>>, fetch: E::Fetch, proxied: bool) -> Syscall {
    let send = move |at: Callsite, request: Request| {
        let fetch = fetch.clone();
        async move {
            let host = request.host.clone();
            tokio::time::timeout_at(at.deadline.into(), fetch.oneshot(request))
                .await
                .with_context(|| format!("{host} did not answer before the call's deadline"))?
        }
    };
    let host = host.clone();
    let secret = move |at: Callsite, secret: String, to: String| {
        let host = host::alive(&host);
        async move { host?.secrets.grant(&at.program, &secret, &to).await }
    };
    match proxied {
        true => http::proxy(send, secret),
        false => http::fetch(send, secret),
    }
}
