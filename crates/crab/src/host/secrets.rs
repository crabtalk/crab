//! Local secret values and service grants, kept in the owner-only secrets file.
use anyhow::{Context, Result};
use berm::clock;
use berm_host::Secrets;
use crabtalk_sdk::{
    account::secret::{Create, Grant, Resource},
    client::LOCAL,
};
use parking_lot::Mutex;
use ring::rand::{SecureRandom, SystemRandom};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs,
    io::{ErrorKind, Write},
    path::{Path, PathBuf},
    sync::Arc,
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

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct Vault {
    services: Table,
    resources: BTreeMap<String, StoredResource>,
}
#[derive(Clone, Serialize, Deserialize)]
struct StoredResource {
    resource: Resource,
    value: String,
}
#[derive(Deserialize)]
#[serde(untagged)]
enum Saved {
    Current(Vault),
    Legacy(Table),
}

/// The file, and the table it holds, read once and written through.
#[derive(Clone)]
pub struct Grants {
    path: PathBuf,
    table: Arc<Mutex<Option<Vault>>>,
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
    fn with<T>(&self, work: impl FnOnce(&mut Vault) -> T) -> Result<T> {
        let mut table = self.table.lock();
        if table.is_none() {
            *table = Some(match fs::read_to_string(&self.path) {
                Ok(text) => match toml::from_str::<Saved>(&text)
                    .with_context(|| format!("{} is not valid", self.path.display()))?
                {
                    Saved::Current(vault) => vault,
                    Saved::Legacy(services) => Vault {
                        services,
                        ..Vault::default()
                    },
                },
                Err(error) if error.kind() == ErrorKind::NotFound => Vault::default(),
                Err(error) => {
                    return Err(error)
                        .with_context(|| format!("failed to read {}", self.path.display()));
                }
            });
        }
        Ok(work(table.as_mut().expect("read above")))
    }

    /// Write the table, readable by its owner alone.
    fn write(&self, table: &Vault) -> Result<()> {
        let text = toml::to_string(table)?;
        let parent = self.path.parent().unwrap_or_else(|| Path::new("."));
        let mut partial = tempfile::NamedTempFile::new_in(parent)?;
        partial.write_all(text.as_bytes())?;
        partial.persist(&self.path).map_err(|error| error.error)?;
        Ok(())
    }

    /// `service`'s secrets, by name, each beside its hosts and when it was
    /// set.
    pub fn list(&self, service: &str) -> Result<Vec<(String, Vec<String>, i64)>> {
        self.with(|table| {
            table
                .services
                .get(service)
                .map(|kept| {
                    kept.iter()
                        .map(|(name, one)| (name.clone(), one.hosts.clone(), one.created_at))
                        .collect()
                })
                .unwrap_or_default()
        })
    }

    fn change<T>(&self, work: impl FnOnce(&mut Vault) -> Result<T>) -> Result<T> {
        self.with(|table| {
            let mut next = table.clone();
            let result = work(&mut next)?;
            self.write(&next)?;
            *table = next;
            Ok(result)
        })?
    }

    pub fn set(&self, service: &str, name: &str, value: &str, hosts: &[String]) -> Result<()> {
        self.change(|table| {
            table.services.entry(service.into()).or_default().insert(
                name.into(),
                Kept {
                    value: value.into(),
                    hosts: hosts.to_vec(),
                    created_at: (clock::now() / 1000) as i64,
                },
            );
            Ok(())
        })
    }

    pub fn remove(&self, service: &str, name: &str) -> Result<bool> {
        self.change(|table| {
            let removed = table
                .services
                .get_mut(service)
                .is_some_and(|kept| kept.remove(name).is_some());
            table.services.retain(|_, kept| !kept.is_empty());
            Ok(removed)
        })
    }

    pub fn resources(&self) -> Result<Vec<Resource>> {
        self.with(|table| {
            table
                .resources
                .values()
                .map(|kept| kept.resource.clone())
                .collect()
        })
    }

    pub fn create_resource(&self, input: &Create) -> Result<Resource> {
        input.check().map_err(anyhow::Error::msg)?;
        let mut random = [0; 16];
        SystemRandom::new()
            .fill(&mut random)
            .map_err(|_| anyhow::anyhow!("random source failed"))?;
        let resource = Resource::new(input.name.clone(), random, (clock::now() / 1000) as i64);
        self.change(|table| {
            table.resources.insert(
                resource.id.clone(),
                StoredResource {
                    resource: resource.clone(),
                    value: input.value.clone(),
                },
            );
            Ok(resource)
        })
    }

    pub fn rotate_resource(&self, id: &str, value: &str) -> Result<bool> {
        self.change(|table| {
            Ok(match table.resources.get_mut(id) {
                Some(kept) => {
                    kept.value = value.into();
                    true
                }
                None => false,
            })
        })
    }

    pub fn delete_resource(&self, id: &str) -> Result<bool> {
        self.change(|table| Ok(table.resources.remove(id).is_some()))
    }

    pub fn grant_resource(&self, id: &str, grant: Grant) -> Result<bool> {
        self.change(|table| {
            Ok(match table.resources.get_mut(id) {
                Some(kept) => {
                    kept.resource
                        .grants
                        .retain(|old| old.service != grant.service);
                    kept.resource.grants.push(grant);
                    true
                }
                None => false,
            })
        })
    }

    pub fn revoke_resource(&self, id: &str, service: &str) -> Result<bool> {
        self.change(|table| {
            Ok(table.resources.get_mut(id).is_some_and(|kept| {
                let before = kept.resource.grants.len();
                kept.resource
                    .grants
                    .retain(|grant| grant.service != service);
                before != kept.resource.grants.len()
            }))
        })
    }
}

impl Secrets for Grants {
    async fn grant(&self, service: &str, secret: &str, host: &str) -> Result<Option<String>> {
        self.with(|table| {
            table
                .services
                .get(service)
                .and_then(|kept| kept.get(secret))
                .filter(|one| one.hosts.iter().any(|to| to == host))
                .map(|one| one.value.clone())
                .or_else(|| {
                    table
                        .resources
                        .get(secret)
                        .filter(|kept| {
                            kept.resource.grants.iter().any(|grant| {
                                grant.service == service && grant.hosts.iter().any(|to| to == host)
                            })
                        })
                        .map(|kept| kept.value.clone())
                })
        })
    }

    async fn resource_owner(&self, service: &str, id: &str, host: &str) -> Result<Option<String>> {
        self.with(|table| {
            table
                .resources
                .get(id)
                .filter(|kept| {
                    kept.resource.grants.iter().any(|grant| {
                        grant.service == service && grant.hosts.iter().any(|to| to == host)
                    })
                })
                .map(|_| String::from(LOCAL))
        })
    }

    async fn names(&self, service: &str) -> Result<Vec<String>> {
        Ok(self
            .list(service)?
            .into_iter()
            .map(|(name, _, _)| name)
            .collect())
    }
}
