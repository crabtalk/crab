//! The memory each scope's services may hold at once, as one [`Pool`] a scope.
//! A service draws from its own scope's pool, whichever chain called it.

use berm::Pool;
use parking_lot::{Mutex, RwLock};
use std::{collections::HashMap, sync::Arc};

pub(crate) struct Pools {
    /// The limit of a scope none was set for, in bytes; `None` is no limit.
    default: Option<u64>,
    set: RwLock<HashMap<String, u64>>,
    /// Each scope's pool, made the first time it is asked for and kept.
    pools: Mutex<HashMap<String, Arc<Pool>>>,
}

impl Pools {
    pub(crate) fn new(default: Option<u64>) -> Self {
        Self {
            default,
            set: RwLock::default(),
            pools: Mutex::default(),
        }
    }

    /// Limit each of `limits`, and every other scope at the default.
    pub(crate) fn replace(&self, limits: HashMap<String, u64>) {
        *self.set.write() = limits;
        for (scope, pool) in self.pools.lock().iter() {
            pool.limit(self.limit(scope));
        }
    }

    /// `scope`'s pool.
    pub(crate) fn pool(&self, scope: &str) -> Arc<Pool> {
        self.pools
            .lock()
            .entry(scope.to_owned())
            .or_insert_with(|| {
                let pool = Arc::new(Pool::default());
                pool.limit(self.limit(scope));
                pool
            })
            .clone()
    }

    fn limit(&self, scope: &str) -> u64 {
        self.set
            .read()
            .get(scope)
            .copied()
            .or(self.default)
            .unwrap_or(0)
    }
}
