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
use std::{
    collections::{BTreeMap, HashMap},
    sync::Arc,
};

/// Where commits go.
#[derive(Clone)]
pub(crate) struct Commits {
    pub(crate) store: Store,
    databases: Arc<Databases>,
    /// What each dry chain's links hold, by chain, until its outermost link on
    /// this host ends.
    pub(crate) kept: Arc<parking_lot::Mutex<HashMap<u64, Kept>>>,
}

/// What one dry chain's ended links hold on this host.
#[derive(Default)]
pub(crate) struct Kept {
    pub(crate) held: Vec<Held>,
    /// The last value a link set under each service's key, for services
    /// without a database.
    pub(crate) writes: HashMap<(String, String), Vec<u8>>,
}

impl Commits {
    pub(crate) fn new(store: Store, databases: Arc<Databases>) -> Self {
        Self {
            store,
            databases,
            kept: Arc::default(),
        }
    }

    /// What an ended link of dry `chain` last set under `program`'s `key`.
    pub(crate) fn kept(&self, chain: u64, program: &str, key: &str) -> Option<Vec<u8>> {
        let kept = self.kept.lock();
        kept.get(&chain)?
            .writes
            .get(&(program.to_owned(), key.to_owned()))
            .cloned()
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
///
/// In a dry chain nothing is committed: each link's hold and writes are kept
/// until the chain's outermost link on this host ends, then every one is
/// rolled back.
pub(crate) fn enter(commits: Commits) -> Enter {
    Arc::new(move |program: &Program, at: &Callsite| {
        let (commits, entering) = (commits.clone(), commits.databases.enter(program, at));
        let first = at.depth == 1;
        let keep = at.dry.as_ref().and_then(|_| {
            let mut kept = commits.kept.lock();
            (!kept.contains_key(&at.chain)).then(|| {
                kept.insert(at.chain, Kept::default());
                Keep {
                    kept: commits.kept.clone(),
                    chain: at.chain,
                }
            })
        });
        let (dry, chain) = (at.dry.is_some(), at.chain);
        Box::pin(async move {
            let held = entering.await?;
            Ok(Box::new(Entered {
                commits,
                held,
                first,
                dry: dry.then_some((chain, keep)),
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
    /// Set in a dry chain: its number, and for its outermost link on this
    /// host, what drops every hold the chain kept.
    pub(crate) dry: Option<(u64, Option<Keep>)>,
}

/// Drops every hold a dry chain kept, which rolls each back.
pub(crate) struct Keep {
    kept: Arc<parking_lot::Mutex<HashMap<u64, Kept>>>,
    chain: u64,
}

impl Drop for Keep {
    fn drop(&mut self) {
        let kept = self.kept.lock().remove(&self.chain);
        drop(kept);
    }
}
