//! What a program says it is, read from its image's [`ABI_SECTION`].

use crate::{ABI_SECTION, ABI_VERSION, ToolSpec};
use anyhow::{Context, Result, bail};
use object::{Object, ObjectSection};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fmt};

#[derive(Debug, Clone, Deserialize)]
pub struct Manifest {
    pub abi_version: u32,
    /// The crate's version when it was built.
    #[serde(default)]
    pub version: String,
    /// The crate's `description`, `homepage` and `repository` from its
    /// `Cargo.toml`.
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub homepage: String,
    #[serde(default)]
    pub repository: String,
    /// The Markdown file `readme` names on `#[program]`.
    #[serde(default)]
    pub readme: String,
    pub tools: Vec<ToolSpec>,
    /// What this program reaches for once it runs.
    ///
    /// Runtime, and resolved wherever the image lands — nothing here is
    /// fetched or installed, and a name nothing answers to is reported rather
    /// than refused. Declared by the author because a target computed at the
    /// call cannot be read off the image at all.
    #[serde(default)]
    pub deps: Deps,
    /// The typed logs it emits.
    #[serde(default)]
    pub events: Vec<EventSpec>,
    /// The SQLite database it keeps, when it keeps one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub database: Option<Database>,
}

/// A program's own SQLite database: the migrations that build it, in order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Database {
    pub migrations: Vec<Migration>,
}

/// One migration. A host applies each once, in order, and refuses a program
/// whose migrations no longer begin with the ones it applied.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Migration {
    pub name: String,
    pub sql: String,
}

/// One typed log a program emits, from an `#[event]` struct. Emitted under the
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

/// What a program reaches for once it runs, by kind.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Deps {
    /// Programs it calls by name.
    #[serde(default)]
    pub programs: Vec<String>,
    /// Syscalls it makes beyond the ABI's own, each beside what it reaches
    /// through that call — the hosts a fetch goes to, the names a secret is
    /// read by. An empty list is the call alone.
    #[serde(default)]
    pub syscalls: BTreeMap<String, Vec<String>>,
    /// Secrets it names in `{secret:NAME}`, which the host substitutes and
    /// the guest never holds.
    #[serde(default)]
    pub secrets: Vec<String>,
}

impl Deps {
    pub fn is_empty(&self) -> bool {
        self.programs.is_empty() && self.syscalls.is_empty() && self.secrets.is_empty()
    }
}

/// `inner, berm.http.fetch (example.com), secret openai` — every program,
/// every syscall with what it reaches, then every secret.
impl fmt::Display for Deps {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut items = self.programs.clone();
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
    /// Read what an image claims to be, without compiling or running it.
    ///
    /// This is what the section is *for* (RFC 0205): learning a program's tools,
    /// deps, and readme must not mean instantiating it. An embedder assembling a
    /// prompt or listing a registry needs exactly this and nothing else.
    pub fn from_image(image: &[u8]) -> Result<Self> {
        let json =
            str::from_utf8(Self::section(image)?).context("program manifest is not UTF-8")?;
        Self::parse(json)
    }

    /// The section's bytes as they sit in the image. A publisher carries these
    /// verbatim, so what a registry serves cannot disagree with what the image
    /// holds.
    pub fn section(image: &[u8]) -> Result<&[u8]> {
        let file = object::File::parse(image).context("program is not a readable image")?;
        file.section_by_name(ABI_SECTION)
            .with_context(|| format!("program has no {ABI_SECTION} section"))?
            .data()
            .context("program manifest is unreadable")
    }

    pub fn parse(json: &str) -> Result<Self> {
        let mut program: Option<Manifest> = None;
        let mut events = Vec::new();
        for record in serde_json::Deserializer::from_str(json).into_iter::<Record>() {
            match record.context("program manifest is not valid JSON")? {
                Record::Event { event } => events.push(event),
                Record::Program(manifest) => {
                    if program.replace(*manifest).is_some() {
                        bail!("program manifest holds two program records");
                    }
                }
            }
        }
        let mut manifest = program.context("program manifest holds no program record")?;
        events.sort_by(|a, b| a.name.cmp(&b.name));
        manifest.events.extend(events);
        if manifest.abi_version != ABI_VERSION {
            bail!(
                "program was built against ABI version {}, this host speaks {ABI_VERSION}",
                manifest.abi_version
            );
        }
        Ok(manifest)
    }
}

/// One record of the manifest section.
#[derive(Deserialize)]
#[serde(untagged)]
enum Record {
    Event { event: EventSpec },
    Program(Box<Manifest>),
}
