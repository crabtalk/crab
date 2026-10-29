//! `berm.call` as this host serves it: a service calling services in its
//! scope, here or on the peer each runs on.

use crate::{
    env::{Env, Observe, Peers},
    host::{self, Inner},
    state::{
        commit::{self, Commits},
        pool::Pools,
    },
};
use anyhow::{Result, bail};
use berm::{Caller, Calls, Callsite, Ended, Meter, Now, Ran};
use berm_api::host::{Kind, Link, NestedCall, Outcome, Reply, Source, User};
use std::{
    sync::{Arc, Weak},
    time::Instant,
};

/// How `host`'s services call services, `depth` deep at most, committing
/// through `commits`, with `observe` told of each invocation, `now` the
/// clock `berm.now` answers, and each service drawing from its scope's pool of
/// `pools`.
pub(crate) fn calls<E: Env>(
    host: &Weak<Inner<E>>,
    depth: u32,
    commits: Commits,
    observe: E::Observe,
    now: Now,
    pools: Arc<Pools>,
) -> Calls {
    Calls {
        depth,
        resolve: {
            let host = host.clone();
            Arc::new(move |at: &Callsite, target: &str, _: &str| {
                let key = resolve(at, target)?;
                let alive = host::alive(&host)?;
                if alive.holds.holds(host::split(&key).0) {
                    bail!("{target} is held");
                }
                if alive.stops.stopped(&key) {
                    bail!("{target} is stopped");
                }
                Ok(key)
            })
        },
        // A call to a service on a peer is counted there.
        called: {
            let host = host.clone();
            Arc::new(
                move |at: &Callsite, deployed: &str, tool: &str, outcome: Outcome| {
                    let Some(host) = host.upgrade() else {
                        return;
                    };
                    if host.berm.get(deployed).is_none() {
                        return;
                    }
                    host.usage.record(
                        deployed,
                        Kind::Tool,
                        tool,
                        Source::Service,
                        &at.service,
                        outcome,
                    );
                },
            )
        },
        // A callee is told the calling service's name; it is in the same
        // scope.
        named: Arc::new(|service: &str| host::split(service).1.to_owned()),
        invoked: {
            let host = host.clone();
            Arc::new(move |at: &Callsite, tool: &str, used: Meter, ran: Ran| {
                let host = host.upgrade();
                let call = host.as_ref().and_then(|host| host.logs.run(at.chain));
                if let (Some(host), Some(call), Some(parent)) = (&host, call, at.parent) {
                    host.calls.nested(
                        call,
                        NestedCall {
                            id: at.span,
                            parent,
                            service: host::split(&at.service).1.to_owned(),
                            tool: tool.to_owned(),
                            at: ran.at,
                            ms: ran.ms,
                            outcome: ran.outcome,
                            cpu: used.cpu,
                            read: used.read,
                            written: used.written,
                        },
                    );
                }
                observe.invoked(&at.service, tool, call, &used, at.spent());
            })
        },
        enter: commit::enter(commits),
        budget: {
            let host = host.clone();
            Arc::new(move |service: &str, tool: &str| {
                host.upgrade()?.terms.get(service, tool).budget
            })
        },
        now,
        remote: {
            let host = host.clone();
            Arc::new(move |at, deployed, tool, args, link| {
                let host = host.upgrade()?;
                let peer = host.routes.peer(deployed)?;
                let from = at.service.to_string();
                let (service, tool) = (deployed.to_owned(), tool.to_owned());
                let link = Link {
                    caller: link.caller.map(|caller| match caller {
                        Caller::Outside(name) | Caller::Service(name) => name.to_string(),
                    }),
                    from,
                    user: link.user.map(|user| User {
                        issuer: user.issuer.to_string(),
                        subject: user.subject.to_string(),
                    }),
                    client: link.client.map(|client| client.to_string()),
                    depth: link.depth,
                    query: link.query,
                    timeout: link
                        .deadline
                        .saturating_duration_since(Instant::now())
                        .as_millis() as u64,
                    stack: link
                        .stack
                        .iter()
                        .map(|service| service.to_string())
                        .collect(),
                };
                let at = at.clone();
                Some(Box::pin(async move {
                    let Reply {
                        result,
                        denied,
                        meters,
                    } = host.peers.link(&peer, &service, &tool, args, link).await?;
                    for (service, tools) in meters {
                        for (tool, used) in tools {
                            at.meter_of(&service, &tool, used);
                        }
                    }
                    Ok(Ended { result, denied })
                }))
            })
        },
        pool: Arc::new(move |service: &str| pools.pool(host::split(service).0)),
    }
}

/// A `berm.call` target: the calling service itself or one its manifest
/// declares in `calls`, by name in the caller's scope.
fn resolve(at: &Callsite, target: &str) -> Result<String> {
    if target.contains('/') {
        bail!("a service calls services in its own scope, by name");
    }
    let (scope, name) = host::split(&at.service);
    if target != name && !at.deps.calls.iter().any(|declared| declared == target) {
        bail!("{name} does not declare {target:?} in its calls");
    }
    Ok(host::join(scope, target))
}
