//! `berm.call` as this host serves it: a service calling services in its
//! scope, here or on the peer each runs on.

use crate::{
    env::{Env, Observe, Peers},
    host::{self, Inner},
    state::commit::{self, Commits},
};
use anyhow::{Result, bail};
use berm::{Caller, Calls, Callsite, Ended, Meter, Now};
use berm_api::host::{Kind, Link, Outcome, Reply, Source, User};
use std::{
    sync::{Arc, Weak},
    time::Instant,
};

/// How `host`'s services call services, `depth` deep at most, committing
/// through `commits`, with `observe` told of each invocation and `now` the
/// clock `berm.now` answers.
pub(crate) fn calls<E: Env>(
    host: &Weak<Inner<E>>,
    depth: u32,
    commits: Commits,
    observe: E::Observe,
    now: Now,
) -> Calls {
    Calls {
        depth,
        resolve: {
            let host = host.clone();
            Arc::new(move |at: &Callsite, target: &str, _: &str| {
                let key = resolve(at, target)?;
                if host::alive(&host)?.holds.holds(host::split(&key).0) {
                    bail!("{target} is held");
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
                        &at.program,
                        outcome,
                    );
                },
            )
        },
        // A callee is told the calling service's name; it is in the same
        // scope.
        named: Arc::new(|program: &str| host::split(program).1.to_owned()),
        invoked: {
            let host = host.clone();
            Arc::new(move |at: &Callsite, tool: &str, used: Meter| {
                let call = host.upgrade().and_then(|host| host.logs.run(at.chain));
                observe.invoked(&at.program, tool, call, &used);
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
                let from = at.program.to_string();
                let (service, tool) = (deployed.to_owned(), tool.to_owned());
                let link = Link {
                    caller: link.caller.map(|caller| match caller {
                        Caller::Outside(name) | Caller::Program(name) => name.to_string(),
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
                        .map(|program| program.to_string())
                        .collect(),
                };
                let at = at.clone();
                Some(Box::pin(async move {
                    let Reply {
                        result,
                        denied,
                        meters,
                    } = host.peers.link(&peer, &service, &tool, args, link).await?;
                    for (service, used) in meters {
                        at.meter_of(&service, used);
                    }
                    Ok(Ended { result, denied })
                }))
            })
        },
    }
}

/// A `berm.call` target: the calling service itself or one its manifest
/// declares in `programs`, by name in the caller's scope.
fn resolve(at: &Callsite, target: &str) -> Result<String> {
    if target.contains('/') {
        bail!("a service calls services in its own scope, by name");
    }
    let (scope, name) = host::split(&at.program);
    if target != name && !at.deps.programs.iter().any(|declared| declared == target) {
        bail!("{name} does not declare {target:?} in its programs");
    }
    Ok(host::join(scope, target))
}
