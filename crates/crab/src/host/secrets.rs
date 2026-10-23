//! The secrets the local host substitutes into its programs' requests: a
//! `secrets.toml` of `NAME = "value"`, one table for every program it runs.
//!
//! A secret reaches only a fetch whose host the program's manifest declares.
//! The file is parsed again when its modification time changes, so an edit
//! applies to the next request.

use anyhow::{Context, Result};
use berm_host::Secrets;
use parking_lot::Mutex;
use std::{collections::BTreeMap, io::ErrorKind, path::PathBuf, sync::Arc, time::SystemTime};

/// What the file is called, at a workspace's root or a host's.
pub const FILE: &str = "secrets.toml";

/// Every secret, by name.
type Table = BTreeMap<String, String>;

/// Where the grants live.
#[derive(Clone)]
pub struct Grants {
    path: PathBuf,
    parsed: Arc<Mutex<Option<Parsed>>>,
}

/// The table last parsed, and the file's modification time when it was.
#[derive(Clone)]
struct Parsed {
    modified: SystemTime,
    table: Arc<Table>,
}

impl Grants {
    /// The secrets in the file at `path`, none while it is absent.
    pub fn open(path: PathBuf) -> Self {
        Self {
            path,
            parsed: Arc::default(),
        }
    }

    /// Every grant, or none when the file is absent.
    async fn read(&self) -> Result<Arc<Table>> {
        let failed = || format!("failed to read {}", self.path.display());
        let modified = match tokio::fs::metadata(&self.path).await {
            Ok(metadata) => metadata.modified().with_context(failed)?,
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(Arc::default()),
            Err(error) => return Err(error).with_context(failed),
        };
        let parsed = self.parsed.lock().clone();
        if let Some(parsed) = parsed
            && parsed.modified == modified
        {
            return Ok(parsed.table);
        }
        let text = match tokio::fs::read_to_string(&self.path).await {
            Ok(text) => text,
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(Arc::default()),
            Err(error) => return Err(error).with_context(failed),
        };
        let table: Arc<Table> = Arc::new(
            toml::from_str(&text)
                .with_context(|| format!("{} is not valid", self.path.display()))?,
        );
        *self.parsed.lock() = Some(Parsed {
            modified,
            table: table.clone(),
        });
        Ok(table)
    }
}

impl Secrets for Grants {
    async fn grant(&self, _: &str, secret: &str, _: &str) -> Result<Option<String>> {
        Ok(self.read().await?.get(secret).cloned())
    }

    async fn names(&self, _: &str) -> Result<Vec<String>> {
        Ok(self.read().await?.keys().cloned().collect())
    }
}
