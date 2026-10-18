//! The secrets a program may have substituted into its requests.
//!
//! Granted in `{root}/secrets.toml`, by program and then by secret name, each
//! beside the hosts it may be sent to:
//!
//! ```toml
//! [fixture.fixture]
//! value = "..."
//! hosts = ["127.0.0.1"]
//! ```
//!
//! Parsed again when the file's modification time changes, so an edit applies
//! to the next request. A grant is the program's whatever starts it.

use anyhow::{Context, Result};
use berm_host::Secrets;
use parking_lot::Mutex;
use serde::Deserialize;
use std::{
    collections::BTreeMap,
    io::ErrorKind,
    path::{Path, PathBuf},
    sync::Arc,
    time::SystemTime,
};

/// What the file is called under the service's root.
const FILE: &str = "secrets.toml";

/// One secret granted to one program.
#[derive(Deserialize)]
struct Grant {
    value: String,
    /// The hosts it may be sent to.
    hosts: Vec<String>,
}

/// Every grant, by program and then by secret name.
type Table = BTreeMap<String, BTreeMap<String, Grant>>;

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
    pub fn open(root: &Path) -> Self {
        Self {
            path: root.join(FILE),
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
    async fn grant(&self, program: &str, secret: &str, host: &str) -> Result<Option<String>> {
        Ok(self
            .read()
            .await?
            .get(program)
            .and_then(|granted| granted.get(secret))
            .filter(|grant| {
                grant
                    .hosts
                    .iter()
                    .any(|allowed| allowed.eq_ignore_ascii_case(host))
            })
            .map(|grant| grant.value.clone()))
    }

    async fn names(&self, program: &str) -> Result<Vec<String>> {
        Ok(self
            .read()
            .await?
            .get(program)
            .map(|granted| granted.keys().cloned().collect())
            .unwrap_or_default())
    }
}
