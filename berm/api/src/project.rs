//! A service's or a kit's project file, [`FILE`], beside its `Cargo.toml`.

use crate::{Access, Credential, DEFAULT_BUFFER, Deps, Limits, ToolRecord, ToolSpec, Var, host};
use anyhow::{Context, Result, bail};
use bytesize::ByteSize;
use regex::Regex;
use serde::Deserialize;
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

/// What the project file is called.
pub const FILE: &str = "crab.toml";

/// The project file. A service's names it; a kit's declares only `id`,
/// `formerly`, `secrets`, `[credentials]`, `calls`, `[syscalls]` and
/// `[database] migrations`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Project {
    /// The kit's ID, 32 lowercase hex digits, which its applied migrations are
    /// kept under. A kit keeping tables declares one; a service declares none.
    #[serde(default)]
    pub id: Option<String>,
    /// What the kit's applied migrations were kept under before `id`: the
    /// package names it had.
    #[serde(default)]
    pub formerly: Vec<String>,
    /// The name it deploys as.
    #[serde(default)]
    pub name: Option<String>,
    /// The public registry repository the image is published to and Cloud
    /// pulls it from, without a tag. Absent, a deploy uploads the image.
    #[serde(default)]
    pub image: Option<String>,
    /// Whether the service is deployed: `false` keeps it out of a
    /// workspace's deploys, and refuses deploying it.
    #[serde(default = "deployed")]
    pub deploy: bool,
    /// Each cron tool's expression, five fields in UTC, such as
    /// `"*/5 * * * *"`. Absent, they are its owners' to set on the host.
    #[serde(default)]
    pub crons: Option<BTreeMap<String, String>>,
    /// What it says of each tool, by name.
    #[serde(default)]
    pub tools: BTreeMap<String, Tool>,
    /// Secrets it names in `{secret:NAME}` that are in no credential.
    #[serde(default)]
    pub secrets: Vec<String>,
    /// Secrets it names in `{secret:NAME}`, grouped by what a person sets up
    /// to get them, by id.
    #[serde(default)]
    pub credentials: BTreeMap<String, Credential>,
    /// Services it calls by name.
    #[serde(default)]
    pub calls: Vec<String>,
    /// The most bytes a call's arguments, and a tool's result, may hold;
    /// [`DEFAULT_BUFFER`] when absent.
    #[serde(default)]
    pub buffer: Option<usize>,
    /// Syscalls it makes beyond the ABI's own, each beside what it reaches
    /// through that call: `"berm.http.fetch" = ["example.com"]`, or `[]` for
    /// the call alone.
    #[serde(default)]
    pub syscalls: BTreeMap<String, Vec<String>>,
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

/// One tool's table in `[tools]` of `crab.toml`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Tool {
    /// Who may call it; absent, it is internal.
    #[serde(default)]
    pub access: Option<Access>,
    /// Its short display name, at most [`ToolSpec::TITLE`] characters on one
    /// line.
    #[serde(default)]
    pub title: Option<String>,
    /// Its price a call on Cloud, in USD micros: `2_000` is $0.002. A deploy
    /// carries it.
    #[serde(default)]
    pub price: Option<u64>,
    /// The catalog job it does, as `company.enrich`.
    #[serde(default)]
    pub catalog: Option<String>,
    /// What one call may use, each key at least 1.
    #[serde(default)]
    pub budget: Option<Limits>,
}

fn deployed() -> bool {
    true
}

impl Project {
    /// The project file at `path`.
    pub fn read(path: &Path) -> Result<Self> {
        let text = fs::read_to_string(path).with_context(|| format!("no {}", path.display()))?;
        let project: Self =
            toml::from_str(&text).with_context(|| format!("cannot parse {}", path.display()))?;
        let mut seen: Vec<&str> = project.secrets.iter().map(String::as_str).collect();
        for secret in project.credentials.values().flat_map(|c| &c.secrets) {
            if seen.contains(&secret.name.as_str()) {
                bail!(
                    "{} declares the secret {} twice",
                    path.display(),
                    secret.name
                );
            }
            seen.push(&secret.name);
        }
        for (name, tool) in &project.tools {
            let at = format!("{}: tools.{name}", path.display());
            if let Some(title) = &tool.title
                && (title.chars().count() > ToolSpec::TITLE || title.contains(['\n', '\r']))
            {
                bail!(
                    "{at}: a title is one line of at most {} characters",
                    ToolSpec::TITLE
                );
            }
            if let Some((key, _)) = tool
                .budget
                .iter()
                .flat_map(Limits::keys)
                .find(|(_, v)| *v == 0)
            {
                bail!("{at}: budget.{key} is at least 1");
            }
            if tool.access.is_none() {
                for (key, set) in [
                    ("price", tool.price.is_some()),
                    ("catalog", tool.catalog.is_some()),
                ] {
                    if set {
                        bail!("{at} sets {key} on an internal tool; give it access");
                    }
                }
            }
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
            for secret in &credential.secrets {
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
            if let Some(host) = credential.hosts.iter().find(|host| !Credential::host(host)) {
                bail!(
                    "{}: credentials.{id} suggests {host:?}, not a host named in full",
                    path.display()
                );
            }
        }
        Ok(project)
    }

    /// The name a service deploys as, refused when it names none or
    /// declares a kit's `id`.
    pub fn name(&self) -> Result<&str> {
        if self.id.is_some() || !self.formerly.is_empty() {
            bail!("only a kit declares an id or formerly");
        }
        self.name
            .as_deref()
            .context("a service names itself: `name = \"…\"`")
    }

    /// What it reaches for once it runs. One keeping a database calls
    /// `berm.sql`, and one declaring vars `berm.var`.
    pub fn deps(&self) -> Deps {
        let mut syscalls = self.syscalls.clone();
        if self.database.migrations.is_some() {
            syscalls.entry("berm.sql".to_owned()).or_default();
        }
        if !self.vars.is_empty() {
            syscalls.entry("berm.var".to_owned()).or_default();
        }
        Deps {
            calls: self.calls.clone(),
            syscalls,
            secrets: self
                .secrets
                .iter()
                .chain(
                    self.credentials
                        .values()
                        .flat_map(|credential| &credential.secrets)
                        .map(|secret| &secret.name),
                )
                .cloned()
                .collect(),
            credentials: self.credentials.clone(),
            vars: self.vars.clone(),
        }
    }

    /// What the build writes of each tool into the service record: all but
    /// its price.
    pub fn records(&self) -> BTreeMap<String, ToolRecord> {
        self.tools
            .iter()
            .map(|(name, tool)| {
                let record = ToolRecord {
                    access: tool.access,
                    title: tool.title.clone(),
                    catalog: tool.catalog.clone(),
                    budget: tool.budget,
                };
                (name.clone(), record)
            })
            .collect()
    }

    /// The most bytes a call's arguments, and a tool's result, may hold.
    pub fn buffer(&self) -> usize {
        self.buffer.unwrap_or(DEFAULT_BUFFER)
    }

    /// Refuse what only a service declares, for a kit's `crab.toml`.
    pub fn check_kit(&self) -> Result<()> {
        let service_only = [
            ("name", self.name.is_some()),
            ("image", self.image.is_some()),
            ("deploy", !self.deploy),
            ("crons", self.crons.is_some()),
            ("tools", !self.tools.is_empty()),
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
                "a kit declares only id, formerly, secrets, [credentials], calls, [syscalls] and [database] migrations, not {}",
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
            None if !self.formerly.is_empty() => {
                bail!("a kit declaring formerly declares its id")
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
