//! Committing what a call wrote. Every invocation commits its own service's
//! writes when it returns ok: a service keeping a database wrote into it as it
//! ran, and the rest goes to the host's store. The chain's first call hands
//! its writes to the host, which commits them.

use crate::{
    host,
    state::{service, store::Store},
};
use anyhow::Result;
use berm::{
    Callsite, Changes, Enter, Program, Scope,
    database::{Databases, Held},
};
use std::{collections::BTreeMap, sync::Arc};

/// Where commits go.
#[derive(Clone)]
pub(crate) struct Commits {
    pub(crate) store: Store,
    databases: Arc<Databases>,
}

impl Commits {
    pub(crate) fn new(store: Store, databases: Arc<Databases>) -> Self {
        Self { store, databases }
    }

    /// Commit `changes`' writes, each service's in one transaction.
    pub(crate) async fn commit(&self, changes: &Changes) -> Result<()> {
        if changes.writes.is_empty() {
            return Ok(());
        }
        let mut shares: BTreeMap<String, Vec<(String, Vec<u8>)>> = BTreeMap::new();
        for write in &changes.writes {
            shares
                .entry(write.program.to_string())
                .or_default()
                .push((write.key.clone(), write.value.clone()));
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
pub(crate) fn enter(commits: Commits) -> Enter {
    Arc::new(move |program: &Program, at: &Callsite| {
        let (commits, entering) = (commits.clone(), commits.databases.enter(program, at));
        let first = at.depth == 1;
        Box::pin(async move {
            let held = entering.await?;
            Ok(Box::new(Entered {
                commits,
                held,
                first,
            }) as Box<dyn Scope>)
        })
    })
}

/// One invocation's hold.
pub(crate) struct Entered {
    pub(crate) commits: Commits,
    pub(crate) held: Held,
    /// The chain's first call, whose changes the host commits.
    pub(crate) first: bool,
}
