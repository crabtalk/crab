//! Outbound HTTP through the daemon's [`Env::Fetch`]. Each request meters
//! its URL, header names, header values and body as [`Meter::sent`].

use crate::{
    env::Env,
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
    FetchAllLiteralBody,
}

/// The syscall `door` names, sent through `fetch` with `host`'s secrets, each
/// request given up at its timeout or its call's deadline, whichever comes
/// first.
pub(crate) fn syscall<E: Env>(host: &Weak<Inner<E>>, fetch: E::Fetch, door: Door) -> Syscall {
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
            tokio::time::timeout_at(given_up.into(), fetch.oneshot(request))
                .await
                .with_context(|| match timeout {
                    Some(timeout) if Some(given_up) == within => {
                        format!("{host} did not answer within {}ms", timeout.as_millis())
                    }
                    _ => format!("{host} did not answer before the call's deadline"),
                })?
        }
    };
    let host = host.clone();
    let secret = move |at: Callsite, secret: String, to: String| {
        let host = host::alive(&host);
        async move { host?.secret(&at, &secret, &to).await }
    };
    match door {
        Door::Fetch => http::fetch(send, secret),
        Door::FetchAll => http::fetch_all(send, secret),
        Door::FetchAllLiteralBody => http::fetch_all_literal_body(send, secret),
    }
}
