//! A service's or a kit's project file, [`FILE`], beside its `Cargo.toml`.

use crate::{
    DEFAULT_BUFFER, Deps, Http, Kit, Var, host, logo::Logo, project::credential::Declaration,
    reference::Reference,
};
use anyhow::{Context, Result, bail};
use bytesize::ByteSize;
use regex::Regex;
use serde::Deserialize;
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

pub mod credential;

/// What the project file is called.
pub const FILE: &str = "crab.toml";

/// A service image's declarations. A library declares only `id`,
/// `[credentials]`, `[http]` and
/// `[database] migrations`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Project {
    /// A linked package's migration ID, 32 lowercase hex digits. A reusable
    /// stateful service declares one alongside its deployment name.
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub image: Option<Image>,
    #[serde(default)]
    pub schedules: BTreeMap<String, Schedule>,
    #[serde(default)]
    pub secret_resources: bool,
    /// Secrets it names in `{secret:NAME}`, grouped by what a person sets up
    /// to get them, by id.
    #[serde(default)]
    pub credentials: BTreeMap<String, Declaration>,
    /// The most bytes a call's arguments, and a tool's result, may hold;
    /// [`DEFAULT_BUFFER`] when absent.
    #[serde(default)]
    pub buffer: Option<usize>,
    #[serde(default)]
    pub http: Http,
    /// Settings each deployed copy sets its own value of, by name.
    #[serde(default)]
    pub vars: BTreeMap<String, Var>,
    #[serde(default)]
    pub database: Database,
}

/// `[database]` of `crab.toml`.
#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Database {
    /// What the service's database may hold, such as `"2GiB"`, from
    /// [`host::MIN_DATABASE_SIZE`] to [`host::MAX_DATABASE_SIZE`] bytes;
    /// [`host::DATABASE_SIZE`] when absent.
    pub size: Option<ByteSize>,
    /// A SQLite database, relative to `crab.toml`, a service on Cloud with no
    /// database yet starts from, such as one the local host filled.
    pub seed: Option<PathBuf>,
    /// A directory of `*.sql` migrations, relative to `crab.toml`, applied in
    /// name order.
    pub migrations: Option<PathBuf>,
}

#[derive(Debug, Clone, Default, serde::Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Workflow {
    #[serde(default)]
    pub schedule: Option<crate::workflow::config::Schedule>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Image {
    pub name: Reference,
    #[serde(default)]
    pub logo: Option<Logo>,
    #[serde(default)]
    pub public: bool,
    #[serde(default)]
    pub deploy: bool,
    /// Whether `crab push` publishes it to `name`.
    #[serde(default = "Image::publish")]
    pub publish: bool,
}

impl Image {
    fn publish() -> bool {
        true
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum Schedule {
    Cron(String),
    Config(ScheduleConfig),
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScheduleConfig {
    pub cron: String,
    pub input: Option<serde_json::Value>,
}

impl Schedule {
    pub fn cron(&self) -> &str {
        match self {
            Self::Cron(cron) => cron,
            Self::Config(config) => &config.cron,
        }
    }
    pub fn input(&self) -> Option<&serde_json::Value> {
        match self {
            Self::Cron(_) => None,
            Self::Config(config) => config.input.as_ref(),
        }
    }
}

impl Project {
    /// The project file at `path`.
    pub fn read(path: &Path) -> Result<Self> {
        let text = fs::read_to_string(path).with_context(|| format!("no {}", path.display()))?;
        let project: Self =
            toml::from_str(&text).with_context(|| format!("cannot parse {}", path.display()))?;
        if let Some(image) = &project.image
            && image.deploy
            && !image.publish
        {
            bail!(
                "{}: image.deploy needs a published image; remove it or image.publish",
                path.display()
            );
        }
        let mut seen: Vec<&str> = Vec::new();
        for secret in project
            .credentials
            .values()
            .flat_map(|c| c.secrets.iter().flatten())
        {
            if seen.contains(&secret.name.as_str()) {
                bail!(
                    "{} declares the secret {} twice",
                    path.display(),
                    secret.name
                );
            }
            seen.push(&secret.name);
        }
        if project.vars.len() > Var::MAX {
            bail!(
                "{} declares {} vars, past the {}",
                path.display(),
                project.vars.len(),
                Var::MAX
            );
        }
        for (name, var) in &project.vars {
            if !Var::name(name) {
                bail!(
                    "{}: vars.{name} is not 1 to 64 ASCII letters, digits or `_`",
                    path.display()
                );
            }
            if var.default.len() > Var::BYTES {
                bail!(
                    "{}: vars.{name}'s default is past {} bytes",
                    path.display(),
                    Var::BYTES
                );
            }
        }
        for (id, credential) in &project.credentials {
            for secret in credential.secrets.iter().flatten() {
                if let Some(pattern) = &secret.pattern
                    && let Err(e) = Regex::new(pattern)
                {
                    bail!(
                        "{}: credentials.{id}: {}'s pattern is not a regular expression: {e}",
                        path.display(),
                        secret.name
                    );
                }
            }
            if let Err(why) =
                crate::Credential::check_hosts(credential.hosts.as_deref().unwrap_or_default())
            {
                bail!("{}: credentials.{id}: {why}", path.display());
            }
        }
        for (id, credential) in &project.credentials {
            for host in credential.hosts.iter().flatten() {
                if project.http.fetch.contains(host) {
                    bail!(
                        "`{host}` is already allowed by credential `{id}`; remove it from `[http] fetch`."
                    );
                }
            }
        }
        for schedule in project.schedules.values() {
            crate::schedule::parse(schedule.cron())?;
        }
        project.deps().check_secret_declarations()?;
        Ok(project)
    }

    pub fn instance<'a>(&'a self, package: &'a str) -> &'a str {
        self.image
            .as_ref()
            .map_or(package, |image| image.name.name.as_str())
    }

    pub fn check_service(&self) -> Result<()> {
        if self.id.is_some() {
            self.check_dependency()?;
        }
        Ok(())
    }

    pub fn deps(&self) -> Deps {
        let mut http = self.http.clone();
        for host in self
            .credentials
            .values()
            .flat_map(|credential| credential.hosts.iter().flatten())
        {
            if !http.fetch.contains(host) {
                http.fetch.push(host.clone());
            }
        }
        let credentials: BTreeMap<_, _> = self
            .credentials
            .iter()
            .filter(|(_, credential)| !credential.hosts_only())
            .map(|(id, credential)| (id.clone(), credential.to_owned().into()))
            .collect();
        Deps {
            http,
            secret_resources: self.secret_resources,
            secrets: credentials
                .values()
                .flat_map(|credential: &crate::Credential| {
                    credential.secrets.iter().map(|secret| secret.name.clone())
                })
                .collect(),
            credentials,
            vars: self.vars.clone(),
        }
    }

    pub fn credential_hosts(&self) -> BTreeMap<String, Vec<String>> {
        self.credentials
            .iter()
            .filter(|(_, credential)| credential.hosts_only())
            .map(|(id, credential)| (id.clone(), credential.hosts.clone().unwrap_or_default()))
            .collect()
    }

    pub fn check_credentials(&self, kits: &[Kit]) -> Result<()> {
        for (id, declaration) in &self.credentials {
            if let Some(kit) = kits
                .iter()
                .find(|kit| kit.deps.credentials.contains_key(id))
            {
                if !declaration.hosts_only() {
                    bail!(
                        "credential `{id}` is declared by kit `{}`; set only `hosts`.",
                        kit.name
                    );
                }
            } else if declaration.title.is_none() || declaration.secrets.is_none() {
                bail!("credential `{id}` has no title or secrets, and no linked kit declares it.");
            }
        }
        Ok(())
    }

    /// The most bytes a call's arguments, and a tool's result, may hold.
    pub fn buffer(&self) -> usize {
        self.buffer.unwrap_or(DEFAULT_BUFFER)
    }

    /// Refuse what only a service declares, for a kit's `crab.toml`.
    pub fn check_kit(&self) -> Result<()> {
        let service_only = [
            ("image", self.image.is_some()),
            ("schedules", !self.schedules.is_empty()),
            ("buffer", self.buffer.is_some()),
            ("vars", !self.vars.is_empty()),
            ("database.size", self.database.size.is_some()),
            ("database.seed", self.database.seed.is_some()),
        ];
        let declared: Vec<&str> = service_only
            .iter()
            .filter(|(_, set)| *set)
            .map(|(key, _)| *key)
            .collect();
        if !declared.is_empty() {
            bail!(
                "a kit declares only id, [credentials], [http] and [database] migrations, not {}",
                declared.join(", ")
            );
        }
        self.check_dependency()
    }

    /// Validate a linked package's migration identity. Deployment settings stay with its own image.
    pub fn check_dependency(&self) -> Result<()> {
        match &self.id {
            None if self.database.migrations.is_some() => {
                bail!("a kit keeping tables declares its id")
            }
            Some(id)
                if id.len() != 32
                    || !id
                        .bytes()
                        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)) =>
            {
                bail!("a kit's id is 32 lowercase hex digits, not {id}")
            }
            _ => Ok(()),
        }
    }
}

impl Database {
    /// What the service's database may hold, in bytes.
    pub fn size(&self) -> u64 {
        self.size.map_or(host::DATABASE_SIZE, |size| size.0)
    }
}
