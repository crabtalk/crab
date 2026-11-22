//! Committing what a call wrote. Every invocation commits its own service's
//! writes when it returns ok: a service keeping a database wrote into it as it
//! ran, and the rest goes to the host's store. The chain's first call hands
//! its writes to the host, which commits them.

use crate::{
    host,
    state::{
        caller::{self, Storage, call::Session},
        service,
        store::Store,
    },
};
use anyhow::Result;
use berm::{
    Callsite, Changes, Enter, Scope, Service, Write,
    database::{Databases, Held},
};
use std::{collections::BTreeMap, sync::Arc};

/// Where commits go.
#[derive(Clone)]
pub(crate) struct Commits {
    pub(crate) store: Store,
    databases: Arc<Databases>,
    storage: Arc<Storage>,
}

impl Commits {
    pub(crate) fn new(store: Store, databases: Arc<Databases>, storage: Arc<Storage>) -> Self {
        Self {
            store,
            databases,
            storage,
        }
    }

    /// Commit `changes`' writes in one transaction, counting a commit of
    /// each service they wrote and of `took`, the service whose database
    /// was committed.
    pub(crate) async fn commit(&self, changes: Changes, took: Option<String>) -> Result<()> {
        if changes.writes.is_empty() && took.is_none() {
            return Ok(());
        }
        let mut shares: BTreeMap<String, Vec<Write>> = BTreeMap::new();
        shares.extend(took.map(|key| (key, Vec::new())));
        for write in changes.writes {
            shares
                .entry(write.service.to_string())
                .or_default()
                .push(write);
        }
        self.store
            .run(move |conn| {
                let tx = conn.transaction()?;
                for (key, writes) in &shares {
                    let (scope, name) = host::split(key);
                    service::commit(&tx, scope, name, writes)?;
                }
                tx.commit()
            })
            .await
    }
}

/// Each invocation holds what [`Databases::enter`] gives it, and its own
/// journal. One that took a service's writer commits that database when it
/// returns ok; unless it is the chain's first, the rest of what it wrote is
/// committed then.
pub(crate) fn enter(commits: Commits, additional: Option<Enter>) -> Enter {
    Arc::new(move |service: &Service, at: &Callsite| {
        let (commits, entering) = (commits.clone(), commits.databases.enter(service, at));
        let admitted = additional.as_ref().map(|enter| enter(service, at));
        let first = at.depth == 1;
        let migrations = service
            .manifest
            .caller_storage
            .as_ref()
            .filter(|_| !caller::scheduled(at))
            .cloned();
        let id = service.storage_id.get().cloned();
        let at = at.clone();
        Box::pin(async move {
            let mut admitted = match admitted {
                Some(admitted) => Some(admitted.await?),
                None => None,
            };
            let entered = async {
                let caller = match migrations {
                    Some(migrations) => Some(
                        commits
                            .storage
                            .enter(&at, id.as_deref(), migrations)
                            .await?,
                    ),
                    None => None,
                };
                Ok::<_, anyhow::Error>((entering.await?, caller))
            }
            .await;
            let (held, caller) = match entered {
                Ok(entered) => entered,
                Err(error) => {
                    if let Some(scope) = &mut admitted {
                        scope.complete().await?;
                    }
                    return Err(error);
                }
            };
            Ok(Box::new(Entered {
                commits,
                held,
                caller,
                admitted,
                first,
            }) as Box<dyn Scope>)
        })
    })
}

/// One invocation's hold.
pub(crate) struct Entered {
    pub(crate) commits: Commits,
    pub(crate) held: Held,
    pub(crate) caller: Option<Session>,
    pub(crate) admitted: Option<Box<dyn Scope>>,
    /// The chain's first call, whose changes the host commits.
    pub(crate) first: bool,
}
