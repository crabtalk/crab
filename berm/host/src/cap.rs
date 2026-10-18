//! How many chains each scope may run at once, beside the host's own gate. A
//! chain takes its scope's slot where it enters the host, from outside, a wake
//! or a Stripe event; a call a peer links into it runs on the slot its chain
//! took.

use parking_lot::{Mutex, RwLock};
use std::{collections::HashMap, sync::Arc};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

pub(crate) struct Caps {
    /// The cap of a scope none was set for; `None` is no cap.
    default: Option<u32>,
    set: RwLock<HashMap<String, u32>>,
    /// Each capped scope's gate, beside the cap it was made for.
    gates: Mutex<HashMap<String, (u32, Arc<Semaphore>)>>,
}

/// A scope's slot, or none for a scope with no cap.
pub(crate) type Slot = Option<OwnedSemaphorePermit>;

impl Caps {
    pub(crate) fn new(default: Option<u32>) -> Self {
        Self {
            default,
            set: RwLock::default(),
            gates: Mutex::default(),
        }
    }

    /// Cap each of `caps`, and every other scope at the default. A scope whose
    /// cap changed gets a new gate; the slots taken of the old run out.
    pub(crate) fn replace(&self, caps: HashMap<String, u32>) {
        *self.set.write() = caps;
    }

    /// A slot of `scope`, `None` when every one is taken.
    pub(crate) fn try_take(&self, scope: &str) -> Option<Slot> {
        match self.gate(scope) {
            Some(gate) => gate.try_acquire_owned().ok().map(Some),
            None => Some(None),
        }
    }

    /// A slot of `scope`, once one is free.
    pub(crate) async fn take(&self, scope: &str) -> Slot {
        let gate = self.gate(scope)?;
        gate.acquire_owned().await.ok()
    }

    fn gate(&self, scope: &str) -> Option<Arc<Semaphore>> {
        let cap = self.set.read().get(scope).copied().or(self.default)?;
        let mut gates = self.gates.lock();
        match gates.get(scope) {
            Some((made, gate)) if *made == cap => Some(gate.clone()),
            _ => {
                let gate = Arc::new(Semaphore::new(cap as usize));
                gates.insert(scope.to_owned(), (cap, gate.clone()));
                Some(gate)
            }
        }
    }
}
