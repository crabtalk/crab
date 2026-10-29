//! What the host keeps: the services it runs, what they commit, and the
//! limits and routes set on them.

use crate::{
    env::Env,
    host::{self, Inner},
};
use berm::{Callsite, Syscall, syscall};
use std::sync::Weak;

pub(crate) mod commit;
pub(crate) mod hold;
pub(crate) mod pool;
pub(crate) mod routes;
mod scope;
pub(crate) mod service;
pub(crate) mod sql;
pub(crate) mod stop;
pub(crate) mod store;
pub(crate) mod terms;
pub(crate) mod vars;

/// `berm.get` and `berm.set` for `host`'s services, over the service's own
/// database when it keeps one and over `service_state` otherwise, and
/// `berm.var` over `service_vars`.
pub(crate) fn syscalls<E: Env>(host: &Weak<Inner<E>>) -> Vec<Syscall> {
    let (reading, writing, var) = (host.clone(), host.clone(), host.clone());
    let mut syscalls = syscall::store::services(
        move |at: Callsite, key: String| {
            let host = host::alive(&reading);
            async move {
                let host = host?;
                if let Some(database) = at.database(|| host.kept(&at.service)) {
                    return database?.get(at.chain, key).await;
                }
                let (scope, name) = host::split(&at.service);
                service::read(&host.store, scope, name, key).await
            }
        },
        move |at: Callsite, key: String, value: Vec<u8>| {
            let host = host::alive(&writing);
            async move {
                let host = host?;
                let Some(database) = at.database(|| host.kept(&at.service)) else {
                    return Ok(false);
                };
                database?.set(at.chain, key, value).await?;
                Ok(true)
            }
        },
    );
    syscalls.push(syscall::var::services(move |at: Callsite, name: String| {
        let host = host::alive(&var);
        async move {
            let host = host?;
            let (scope, service) = host::split(&at.service);
            vars::read(&host.store, scope, service, name).await
        }
    }));
    syscalls
}
