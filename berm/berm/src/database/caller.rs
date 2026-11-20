use crate::{
    Callsite,
    database::{Database, Held},
};
use anyhow::Result;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

#[derive(Debug)]
pub struct CallerDatabase {
    pub database: Arc<Database>,
    pub authenticated: bool,
    wrote: AtomicBool,
}

impl CallerDatabase {
    pub fn new(database: Arc<Database>, authenticated: bool) -> Self {
        Self {
            database,
            authenticated,
            wrote: AtomicBool::new(false),
        }
    }

    pub fn wrote(&self) -> bool {
        self.wrote.load(Ordering::Relaxed)
    }

    pub fn mark_write(&self) {
        self.wrote.store(true, Ordering::Relaxed);
    }

    pub async fn enter(&self, at: &Callsite) -> Result<Held> {
        Ok(Held {
            writing: if at.query {
                None
            } else {
                Some(self.database.write(at.chain, at.deadline).await?)
            },
            marked: Vec::new(),
        })
    }
}
