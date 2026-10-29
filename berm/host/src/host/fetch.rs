//! `berm.http.fetch`, `berm.http.fetch_all`, `berm.http.proxy` and
//! `berm.http.proxy_all`, sent through the daemon's [`Env::Fetch`]. Each
//! request sent meters [`Meter::sent`], and each answered request through the
//! proxy meters [`Meter::proxied`]: its URL, header names and values and body,
//! and the response's header names and values and body.

use crate::{
    env::{Env, Secrets},
    host::{self, Inner},
};
use anyhow::Context;
use berm::{
    Callsite, Meter, Syscall,
    syscall::http::{self, Request},
};
use std::{sync::Weak, time::Instant};
use tower::ServiceExt;

/// Which of the HTTP syscalls is served.
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Door {
    Fetch,
    FetchAll,
    Proxy,
    ProxyAll,
}

/// The syscall `door` names, sent through `fetch` with `host`'s secrets, each
/// request given up at its timeout or its call's deadline, whichever comes
/// first.
pub(crate) fn syscall<E: Env>(host: &Weak<Inner<E>>, fetch: E::Fetch, door: Door) -> Syscall {
    let proxied = matches!(door, Door::Proxy | Door::ProxyAll);
    let send = move |at: Callsite, request: Request| {
        let fetch = fetch.clone();
        async move {
            let size = request.size();
            at.meter(Meter {
                sent: size as u64,
                ..Meter::default()
            });
            let host = request.host.clone();
            let timeout = request.timeout;
            let within = timeout.map(|timeout| Instant::now() + timeout);
            let given_up = within.map_or(at.deadline, |within| within.min(at.deadline));
            let response = tokio::time::timeout_at(given_up.into(), fetch.oneshot(request))
                .await
                .with_context(|| match timeout {
                    Some(timeout) if Some(given_up) == within => {
                        format!("{host} did not answer within {}ms", timeout.as_millis())
                    }
                    _ => format!("{host} did not answer before the call's deadline"),
                })??;
            if proxied {
                at.meter(Meter {
                    proxied: (size + response.size()) as u64,
                    ..Meter::default()
                });
            }
            Ok(response)
        }
    };
    let host = host.clone();
    let secret = move |at: Callsite, secret: String, to: String| {
        let host = host::alive(&host);
        async move { host?.secrets.grant(&at.program, &secret, &to).await }
    };
    match door {
        Door::Fetch => http::fetch(send, secret),
        Door::FetchAll => http::fetch_all(send, secret),
        Door::Proxy => http::proxy(send, secret),
        Door::ProxyAll => http::proxy_all(send, secret),
    }
}
