//! The secrets the local host substitutes into its services' requests, kept
//! by service in `secrets.toml` under its root: `[service]` then
//! `NAME = { value = "…", hosts = ["…"], created_at = … }`.
//!
//! Only the host writes the file, readable by its owner alone. A secret
//! reaches only a fetch to one of its hosts that the service's manifest
//! declares.

use anyhow::{Context, Result};
use berm_host::Secrets;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::{
    collections::BTreeMap,
    fs,
    io::ErrorKind,
    path::PathBuf,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

/// What the file is called under the host's root.
pub const FILE: &str = "secrets.toml";

/// One secret of a service.
#[derive(Clone, Serialize, Deserialize)]
pub struct Kept {
    pub value: String,
    /// The hosts it is sent to; none for one set before hosts were kept.
    #[serde(default)]
    pub hosts: Vec<String>,
    /// When it was set, in Unix seconds.
    pub created_at: i64,
}

/// Every secret, by service and then by name.
type Table = BTreeMap<String, BTreeMap<String, Kept>>;

/// The file, and the table it holds, read once and written through.
#[derive(Clone)]
pub struct Grants {
    path: PathBuf,
    table: Arc<Mutex<Option<Table>>>,
}

impl Grants {
    /// The secrets in the file at `path`, none while it is absent.
    pub fn open(path: PathBuf) -> Self {
        Self {
            path,
            table: Arc::default(),
        }
    }

    /// Run `work` on the table, read from the file the first time.
    fn with<T>(&self, work: impl FnOnce(&mut Table) -> T) -> Result<T> {
        let mut table = self.table.lock();
        if table.is_none() {
            *table = Some(match fs::read_to_string(&self.path) {
                Ok(text) => toml::from_str(&text)
                    .with_context(|| format!("{} is not valid", self.path.display()))?,
                Err(error) if error.kind() == ErrorKind::NotFound => Table::new(),
                Err(error) => {
                    return Err(error)
                        .with_context(|| format!("failed to read {}", self.path.display()));
                }
            });
        }
        Ok(work(table.as_mut().expect("read above")))
    }

    /// Write the table, readable by its owner alone.
    fn write(&self, table: &Table) -> Result<()> {
        let text = toml::to_string(table)?;
        let partial = self.path.with_extension("toml.partial");
        fs::write(&partial, text).with_context(|| format!("cannot write {}", partial.display()))?;
        #[cfg(unix)]
        fs::set_permissions(&partial, fs::Permissions::from_mode(0o600))?;
        fs::rename(&partial, &self.path)
            .with_context(|| format!("cannot write {}", self.path.display()))
    }

    /// `service`'s secrets, by name, each beside its hosts and when it was
    /// set.
    pub fn list(&self, service: &str) -> Result<Vec<(String, Vec<String>, i64)>> {
        self.with(|table| {
            table
                .get(service)
                .map(|kept| {
                    kept.iter()
                        .map(|(name, one)| (name.clone(), one.hosts.clone(), one.created_at))
                        .collect()
                })
                .unwrap_or_default()
        })
    }

    /// Set `service`'s secret `name` to `value`, sent to `hosts`; `false`
    /// when it is set already.
    pub fn set(&self, service: &str, name: &str, value: &str, hosts: &[String]) -> Result<bool> {
        let created_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |since| since.as_secs() as i64);
        let table = self.with(|table| {
            let kept = table.entry(service.to_owned()).or_default();
            if kept.contains_key(name) {
                return None;
            }
            kept.insert(
                name.to_owned(),
                Kept {
                    value: value.to_owned(),
                    hosts: hosts.to_vec(),
                    created_at,
                },
            );
            Some(table.clone())
        })?;
        match table {
            Some(table) => self.write(&table).map(|()| true),
            None => Ok(false),
        }
    }

    /// Send `service`'s secret `name` to `hosts` alone; `false` when it is
    /// not set.
    pub fn set_hosts(&self, service: &str, name: &str, hosts: &[String]) -> Result<bool> {
        let table = self.with(|table| {
            let one = table.get_mut(service)?.get_mut(name)?;
            one.hosts = hosts.to_vec();
            Some(table.clone())
        })?;
        match table {
            Some(table) => self.write(&table).map(|()| true),
            None => Ok(false),
        }
    }

    /// Revoke `service`'s secret `name`; `false` when it is not set.
    pub fn remove(&self, service: &str, name: &str) -> Result<bool> {
        let table = self.with(|table| {
            let removed = table
                .get_mut(service)
                .is_some_and(|kept| kept.remove(name).is_some());
            table.retain(|_, kept| !kept.is_empty());
            removed.then(|| table.clone())
        })?;
        match table {
            Some(table) => self.write(&table).map(|()| true),
            None => Ok(false),
        }
    }
}

impl Secrets for Grants {
    async fn grant(&self, service: &str, secret: &str, host: &str) -> Result<Option<String>> {
        self.with(|table| {
            table
                .get(service)
                .and_then(|kept| kept.get(secret))
                .filter(|one| one.hosts.iter().any(|to| to == host))
                .map(|one| one.value.clone())
        })
    }

    async fn names(&self, service: &str) -> Result<Vec<String>> {
        Ok(self
            .list(service)?
            .into_iter()
            .filter(|(_, hosts, _)| !hosts.is_empty())
            .map(|(name, _, _)| name)
            .collect())
    }
}
