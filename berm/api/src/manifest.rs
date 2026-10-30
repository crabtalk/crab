//! What a service says it is, read from its image's [`ABI_SECTION`],
//! [`SERVICE_SECTION`] and [`KITS_SECTION`].

use crate::{Access, DEFAULT_BUFFER, Image, Limits, ToolSpec};
use anyhow::{Context, Result, bail};
use berm_abi::{ABI_SECTION, ABI_VERSION, KITS_SECTION, SERVICE_SECTION};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fmt, str::FromStr};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    pub abi_version: u32,
    /// The package it was built from.
    #[serde(default)]
    pub name: String,
    /// The package's version when it was built.
    #[serde(default)]
    pub version: String,
    /// The package's `description`, `homepage` and `repository` from its
    /// `Cargo.toml`.
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub homepage: String,
    #[serde(default)]
    pub repository: String,
    /// The Markdown file `readme` names in its `crab.toml`.
    #[serde(default)]
    pub readme: String,
    /// By name.
    pub tools: Vec<ToolSpec>,
    /// The most bytes a call's arguments, and a tool's result, may hold. The
    /// host refuses arguments past it and fails a result past it.
    #[serde(default = "default_buffer")]
    pub buffer: usize,
    /// What this service reaches for once it runs, its kits' declarations
    /// included.
    ///
    /// Resolved wherever the image lands; nothing here is fetched or
    /// installed, and a name nothing answers to is reported, not refused.
    #[serde(default)]
    pub deps: Deps,
    /// The typed logs it emits, by name.
    #[serde(default)]
    pub events: Vec<EventSpec>,
    /// The SQLite database it keeps, when it keeps one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub database: Option<Database>,
    /// The kits it links, by name.
    #[serde(default)]
    pub kits: Vec<Kit>,
}

/// The [`SERVICE_SECTION`] the build appends: the service's own declarations,
/// from its package and its `crab.toml`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceRecord {
    /// The package it was built from. Each tool record names it.
    pub name: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub homepage: String,
    #[serde(default)]
    pub repository: String,
    #[serde(default)]
    pub readme: String,
    pub buffer: usize,
    #[serde(default)]
    pub deps: Deps,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub database: Option<Database>,
    /// What `[tools]` in its `crab.toml` says of each tool, by name.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub tools: BTreeMap<String, ToolRecord>,
}

/// What `[tools]` in a service's `crab.toml` says of one tool, applied onto
/// its [`ToolSpec`].
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolRecord {
    /// `None` is internal.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub access: Option<Access>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub catalog: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub budget: Option<Limits>,
}

/// A library crate a service links that has a `crab.toml` of its own: its
/// declarations, as one entry of the [`KITS_SECTION`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Kit {
    /// The kit's package.
    pub name: String,
    /// The kit's ID from its `crab.toml`, which its applied migrations are kept
    /// under. Present on a kit with a `database`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// What its applied migrations were kept under before `id`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub formerly: Vec<String>,
    /// The package's version when the service was built.
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub deps: Deps,
    /// The tables it keeps in the service's database.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub database: Option<Database>,
}

/// A service's own SQLite database: the migrations that build it, in order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Database {
    pub migrations: Vec<Migration>,
}

/// One sequence of migrations a database is built by.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sequence {
    /// What its applied migrations are kept under: a kit's id, or `""` for
    /// the service's own.
    pub kit: String,
    /// What its applied migrations were kept under before `kit`.
    pub formerly: Vec<String>,
    pub migrations: Vec<Migration>,
}

/// One migration. A host applies each once, in order, and refuses a service
/// whose migrations no longer begin with the ones it applied.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Migration {
    pub name: String,
    pub sql: String,
}

/// One typed log a service emits, from an `#[event]` struct. Emitted under the
/// topic `name`, its data the JSON `fields` describes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventSpec {
    pub name: String,
    pub description: String,
    /// JSON Schema for the log's data.
    pub fields: serde_json::Value,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub deprecated: bool,
}

/// What a service reaches for once it runs, by kind.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Deps {
    /// Services it calls by name.
    #[serde(default)]
    pub calls: Vec<String>,
    /// Syscalls it makes beyond the ABI's own, each beside what it reaches
    /// through that call — the hosts a fetch goes to, the names a secret is
    /// read by. An empty list is the call alone.
    #[serde(default)]
    pub syscalls: BTreeMap<String, Vec<String>>,
    /// Secrets it names in `{secret:NAME}`, which the host substitutes and
    /// the guest never holds. Every credential's secrets are among them.
    #[serde(default)]
    pub secrets: Vec<String>,
    /// How a person gets its secrets, grouped by what they set up, by id. A
    /// secret is in one credential at most.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub credentials: BTreeMap<String, Credential>,
    /// Settings each deployed copy holds its own value of, read through
    /// `berm.var`, by name.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub vars: BTreeMap<String, Var>,
}

/// One var of `[vars]` in a service's `crab.toml`.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Var {
    /// What `berm.var` answers for a copy that set no value.
    #[serde(default)]
    pub default: String,
    /// What the deployer sets it to, shown where they set it.
    #[serde(default)]
    pub description: String,
}

impl Var {
    /// The most vars one service declares.
    pub const MAX: usize = 32;
    /// The most bytes a value or a default holds.
    pub const BYTES: usize = 4096;

    /// Whether `name` names a var: 1 to 64 ASCII letters, digits and `_`.
    pub fn name(name: &str) -> bool {
        (1..=64).contains(&name.len())
            && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
    }
}

/// Secrets a person gets together from one provider, such as a GitHub OAuth
/// app's client ID and secret.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Credential {
    /// What the person sets up, as `GitHub OAuth app`.
    pub title: String,
    /// How to get its secrets, in Markdown.
    #[serde(default)]
    pub guide: String,
    /// Values the person enters at the provider, each label by key. A secret
    /// request gives each value, since they differ per product.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub copy: BTreeMap<String, String>,
    /// The hosts its secrets are suggested to be sent to, each named in full.
    /// A secret goes only to the hosts the person setting it confirms.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hosts: Vec<String>,
    /// Whether the service is called with its secrets unset. A header naming
    /// an unset one is left off the request; a body naming one is refused.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub optional: bool,
    /// In the order the person sets them.
    pub secrets: Vec<CredentialSecret>,
}

impl Credential {
    /// Whether `host` names one host in full, as a credential's suggested
    /// hosts and a set secret's hosts do: lowercase ASCII letters, digits,
    /// `-` and `.`, with no pattern, port or scheme.
    pub fn host(host: &str) -> bool {
        !host.is_empty()
            && host.len() <= 253
            && !host.starts_with(['.', '-'])
            && !host.ends_with(['.', '-'])
            && !host.contains("..")
            && host
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'.')
    }
}

/// One secret of a [`Credential`].
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CredentialSecret {
    pub name: String,
    /// What the provider calls it, as `Client ID`; the name when empty.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub label: String,
    /// Never shown once set. An unmasked one, such as a client ID, is.
    #[serde(default = "masked")]
    pub masked: bool,
    /// A regular expression a value must match; one that does not is refused.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pattern: Option<String>,
}

fn masked() -> bool {
    true
}

impl Deps {
    /// Refuse setting `var` to `value`: a var `[vars]` does not declare, or
    /// a value past [`Var::BYTES`].
    pub fn settable(&self, var: &str, value: &str) -> Result<()> {
        if !self.vars.contains_key(var) {
            bail!("{var} is not declared in [vars]");
        }
        if value.len() > Var::BYTES {
            bail!(
                "a var holds at most {} bytes, and {var}'s value is {}",
                Var::BYTES,
                value.len()
            );
        }
        Ok(())
    }

    pub fn is_empty(&self) -> bool {
        self.calls.is_empty()
            && self.syscalls.is_empty()
            && self.secrets.is_empty()
            && self.vars.is_empty()
    }

    /// Add what `other` reaches that this does not. Refused when both declare
    /// a credential of one id, or one secret in two credentials.
    pub fn merge(&mut self, other: &Deps) -> Result<()> {
        let missing = |into: &mut Vec<String>, from: &[String]| {
            for item in from {
                if !into.contains(item) {
                    into.push(item.clone());
                }
            }
        };
        missing(&mut self.calls, &other.calls);
        missing(&mut self.secrets, &other.secrets);
        for (name, reached) in &other.syscalls {
            missing(self.syscalls.entry(name.clone()).or_default(), reached);
        }
        for (id, credential) in &other.credentials {
            if self.credentials.contains_key(id) {
                bail!("credential {id} is declared twice");
            }
            if let Some(secret) = credential
                .secrets
                .iter()
                .find(|secret| self.credential_of(&secret.name).is_some())
            {
                bail!("secret {} is in two credentials", secret.name);
            }
            self.credentials.insert(id.clone(), credential.clone());
        }
        Ok(())
    }

    /// Every credential, then each secret in none as a credential of its
    /// own, by the secret's name, with no title or guide.
    pub fn grouped(&self) -> Vec<(String, Credential)> {
        let alone = self
            .secrets
            .iter()
            .filter(|name| self.credential_of(name).is_none())
            .map(|name| {
                let credential = Credential {
                    title: String::new(),
                    guide: String::new(),
                    copy: BTreeMap::new(),
                    hosts: Vec::new(),
                    optional: false,
                    secrets: vec![CredentialSecret {
                        name: name.clone(),
                        label: String::new(),
                        masked: true,
                        pattern: None,
                    }],
                };
                (name.clone(), credential)
            });
        self.credentials
            .iter()
            .map(|(id, credential)| (id.clone(), credential.clone()))
            .chain(alone)
            .collect()
    }

    /// Refused when `value` does not match the pattern a credential declares
    /// for `secret`. A secret no pattern covers takes any value.
    pub fn admits(&self, secret: &str, value: &str) -> Result<(), String> {
        let Some(declared) = self
            .credentials
            .values()
            .flat_map(|credential| &credential.secrets)
            .find(|declared| declared.name == secret)
        else {
            return Ok(());
        };
        let Some(pattern) = &declared.pattern else {
            return Ok(());
        };
        let compiled =
            Regex::new(pattern).map_err(|e| format!("{secret}'s pattern is not one: {e}"))?;
        match compiled.is_match(value) {
            true => Ok(()),
            false => {
                let label = match declared.label.is_empty() {
                    true => secret,
                    false => &declared.label,
                };
                Err(format!("{label} does not match {pattern}"))
            }
        }
    }

    /// The id of the credential `secret` is in, when it is in one.
    pub fn credential_of(&self, secret: &str) -> Option<&str> {
        self.credentials
            .iter()
            .find(|(_, credential)| credential.secrets.iter().any(|s| s.name == secret))
            .map(|(id, _)| id.as_str())
    }

    /// Whether `secret` is in an optional credential.
    pub fn optional(&self, secret: &str) -> bool {
        self.credential_of(secret)
            .is_some_and(|id| self.credentials[id].optional)
    }

    /// How `host` is declared under `syscall`, or `None` when it is not.
    ///
    /// A declared host is a name, matched whole; `*.example.com`, matching
    /// every subdomain of `example.com` and not `example.com` itself; or `*`,
    /// matching every host. Matching ignores ASCII case. A host declared by
    /// name is [`Reach::Named`] even when a pattern matches it too.
    pub fn reach(&self, syscall: &str, host: &str) -> Option<Reach> {
        let declared = self.syscalls.get(syscall)?;
        if declared.iter().any(|item| item.eq_ignore_ascii_case(host)) {
            return Some(Reach::Named);
        }
        declared
            .iter()
            .any(|item| match item.strip_prefix("*") {
                Some("") => true,
                Some(suffix) => {
                    suffix.starts_with('.')
                        && host.len().checked_sub(suffix.len()).is_some_and(|at| {
                            host.as_bytes()[at..].eq_ignore_ascii_case(suffix.as_bytes())
                        })
                }
                None => false,
            })
            .then_some(Reach::Pattern)
    }
}

/// How a manifest declares a host it reaches.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reach {
    /// By its name.
    Named,
    /// Only through `*` or `*.domain`.
    Pattern,
}

/// `inner, berm.http.fetch (example.com), secret openai` — every service,
/// every syscall with what it reaches, then every secret.
impl fmt::Display for Deps {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut items = self.calls.clone();
        items.extend(
            self.syscalls
                .iter()
                .map(|(name, reached)| match reached.is_empty() {
                    true => name.clone(),
                    false => format!("{name} ({})", reached.join(", ")),
                }),
        );
        items.extend(self.secrets.iter().map(|name| format!("secret {name}")));
        f.write_str(&items.join(", "))
    }
}

impl Manifest {
    /// Whether it keeps a database: its own, or a kit's tables.
    pub fn keeps_database(&self) -> bool {
        self.database.is_some() || self.kits.iter().any(|kit| kit.database.is_some())
    }

    /// Each sequence of migrations its database is built by, under the name
    /// its applied migrations are kept by: each kit's under the kit's, in
    /// name order, then its own under `""`.
    pub fn migrations(&self) -> Vec<Sequence> {
        let kits = self.kits.iter().filter_map(|kit| {
            let database = kit.database.as_ref()?;
            Some(Sequence {
                kit: kit.id.clone()?,
                formerly: kit.formerly.clone(),
                migrations: database.migrations.clone(),
            })
        });
        let own = self.database.iter().map(|database| Sequence {
            kit: String::new(),
            formerly: Vec::new(),
            migrations: database.migrations.clone(),
        });
        kits.chain(own).collect()
    }

    /// Refuse a manifest of another ABI, with no tools, naming a kit twice,
    /// with a kit keeping tables or naming former keys and declaring no id, or
    /// with an id or former key empty or named twice.
    fn checked(self) -> Result<Self> {
        versioned(self.abi_version)?;
        if self.tools.is_empty() {
            bail!("service declares no tools");
        }
        for (at, kit) in self.kits.iter().enumerate() {
            if self.kits[..at].iter().any(|seen| seen.name == kit.name) {
                bail!("service manifest names kit {} twice", kit.name);
            }
            if kit.database.is_some() && kit.id.is_none() {
                bail!("kit {} keeps tables and declares no id", kit.name);
            }
            if kit.formerly.iter().any(|key| key.is_empty()) {
                bail!("kit {} names an empty former key", kit.name);
            }
            if !kit.formerly.is_empty() && kit.id.is_none() {
                bail!("kit {} names former keys and declares no id", kit.name);
            }
            let keys = |kit: &Kit| -> Vec<String> {
                kit.id.iter().chain(&kit.formerly).cloned().collect()
            };
            let mine = keys(kit);
            if let Some(key) = mine
                .iter()
                .enumerate()
                .find_map(|(at, key)| mine[..at].contains(key).then_some(key))
            {
                bail!("kit {} names the key {key} twice", kit.name);
            }
            if let Some(key) = mine
                .iter()
                .find(|key| self.kits[..at].iter().any(|seen| keys(seen).contains(key)))
            {
                bail!("kits {} and another keep migrations under {key}", kit.name);
            }
        }
        Ok(self)
    }
}

/// What an image claims to be, read without compiling or running it.
///
/// Each tool takes its access, title and catalog from the service record.
/// Refused when the image has no [`SERVICE_SECTION`], which a bare
/// `cargo build` leaves out; when it has no tools; when a tool record was
/// compiled in a crate other than the service's; when the service record
/// names a tool the image lacks, or gives a cron or stripe tool access; and
/// when it names a kit twice.
impl TryFrom<&[u8]> for Manifest {
    type Error = anyhow::Error;

    fn try_from(image: &[u8]) -> Result<Self> {
        let image = Image(image);
        let abi = image.section(ABI_SECTION)?.unwrap_or_default();
        let abi =
            str::from_utf8(abi).with_context(|| format!("service's {ABI_SECTION} is not UTF-8"))?;
        let mut tools = Vec::new();
        let mut events = Vec::new();
        for record in serde_json::Deserializer::from_str(abi).into_iter::<AbiRecord>() {
            match record.with_context(|| format!("service's {ABI_SECTION} is not valid JSON"))? {
                AbiRecord::Event { event } => events.push(event),
                AbiRecord::Tool {
                    tool,
                    abi_version,
                    krate,
                } => {
                    versioned(abi_version)?;
                    tools.push((tool, krate));
                }
                AbiRecord::Service { abi_version } => {
                    versioned(abi_version)?;
                    bail!("service's {ABI_SECTION} holds a service record");
                }
            }
        }

        let service = image.section(SERVICE_SECTION)?.with_context(|| {
            format!("service has no {SERVICE_SECTION} section, which `crab build` appends")
        })?;
        let mut service: ServiceRecord = serde_json::from_slice(service)
            .with_context(|| format!("service's {SERVICE_SECTION} is not valid"))?;
        let kits: Vec<Kit> = match image.section(KITS_SECTION)? {
            Some(kits) => serde_json::from_slice(kits)
                .with_context(|| format!("service's {KITS_SECTION} is not valid"))?,
            None => Vec::new(),
        };

        let mut records = std::mem::take(&mut service.tools);
        let tools = tools
            .into_iter()
            .map(|(mut tool, krate)| {
                if krate != service.name {
                    bail!(
                        "tool {:?} was compiled in {krate}, not in the service {}; a kit carries no tools",
                        tool.name,
                        service.name
                    );
                }
                let record = records.remove(&tool.name).unwrap_or_default();
                if record.access.is_some() && (tool.cron || tool.stripe) {
                    bail!(
                        "tool {:?} is called by its host, and takes no access",
                        tool.name
                    );
                }
                tool.set_access(record.access);
                tool.title = record.title;
                tool.catalog = record.catalog;
                tool.budget = record.budget;
                Ok(tool)
            })
            .collect::<Result<Vec<_>>>()?;
        if let Some(name) = records.keys().next() {
            bail!("[tools] names {name:?}, which the service has no tool of");
        }

        let ServiceRecord {
            name,
            version,
            description,
            homepage,
            repository,
            readme,
            buffer,
            mut deps,
            database,
            tools: _,
        } = service;
        for kit in &kits {
            deps.merge(&kit.deps)
                .with_context(|| format!("linking kit {}", kit.name))?;
        }
        let mut manifest = Manifest {
            abi_version: ABI_VERSION,
            name,
            version,
            description,
            homepage,
            repository,
            readme,
            tools,
            buffer,
            deps,
            events,
            database,
            kits,
        };
        manifest.tools.sort_by(|a, b| a.name.cmp(&b.name));
        manifest.events.sort_by(|a, b| a.name.cmp(&b.name));
        manifest.checked()
    }
}

/// A manifest as JSON, as [`Manifest`] serializes it: what a registry carries
/// beside the image.
impl FromStr for Manifest {
    type Err = anyhow::Error;

    fn from_str(json: &str) -> Result<Self> {
        let manifest: Manifest =
            serde_json::from_str(json).context("service manifest is not valid JSON")?;
        manifest.checked()
    }
}

/// Refuse `abi_version` unless it is the one this host speaks.
fn versioned(abi_version: u32) -> Result<()> {
    if abi_version != ABI_VERSION {
        bail!(
            "service was built against ABI version {abi_version}, this host speaks {ABI_VERSION}"
        );
    }
    Ok(())
}

fn default_buffer() -> usize {
    DEFAULT_BUFFER
}

/// One record of the [`ABI_SECTION`].
#[derive(Deserialize)]
#[serde(untagged)]
enum AbiRecord {
    Event {
        event: EventSpec,
    },
    /// One per `#[tool]`, naming the package it was compiled in.
    Tool {
        tool: ToolSpec,
        abi_version: u32,
        #[serde(rename = "crate")]
        krate: String,
    },
    /// The service record an image built against ABI version 4 or earlier
    /// carries in its place.
    Service {
        abi_version: u32,
    },
}
