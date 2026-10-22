//! `berm.toml`, beside the `Cargo.toml` of a program or a kit.

use crate::{DEFAULT_BUFFER, Deps, host};
use anyhow::{Context, Result, bail};
use bytesize::ByteSize;
use serde::Deserialize;
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

/// `berm.toml`. A program's names its service; a kit's declares only
/// `secrets`, `programs`, `[syscalls]` and `[database] migrations`.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Project {
    /// The service the program deploys as.
    #[serde(default)]
    pub name: Option<String>,
    /// The registry repository Cloud pulls the image from, without a tag.
    #[serde(default)]
    pub image: Option<String>,
    /// The tools the program may make public.
    #[serde(default)]
    pub public: Vec<String>,
    /// Each cron tool's expression, five fields in UTC, such as
    /// `"*/5 * * * *"`. Absent, they are its owners' to set on the host.
    #[serde(default)]
    pub crons: Option<BTreeMap<String, String>>,
    /// Secrets it names in `{secret:NAME}`.
    #[serde(default)]
    pub secrets: Vec<String>,
    /// Programs it calls by name.
    #[serde(default)]
    pub programs: Vec<String>,
    /// The most bytes a call's arguments, and a tool's result, may hold;
    /// [`DEFAULT_BUFFER`] when absent.
    #[serde(default)]
    pub buffer: Option<usize>,
    /// A Markdown file, relative to `berm.toml`, the manifest carries as the
    /// program's README; none when absent.
    #[serde(default)]
    pub readme: Option<PathBuf>,
    /// Syscalls it makes beyond the ABI's own, each beside what it reaches
    /// through that call: `"berm.http.fetch" = ["example.com"]`, or `[]` for
    /// the call alone.
    #[serde(default)]
    pub syscalls: BTreeMap<String, Vec<String>>,
    #[serde(default)]
    pub database: Database,
}

/// `[database]` of `berm.toml`.
#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Database {
    /// What the service's database may hold, such as `"2GiB"`, from
    /// [`host::MIN_DATABASE_SIZE`] to [`host::MAX_DATABASE_SIZE`] bytes;
    /// [`host::DATABASE_SIZE`] when absent.
    pub size: Option<ByteSize>,
    /// A SQLite database, relative to `berm.toml`, a service on Cloud with no
    /// database yet starts from, such as one a bermd filled.
    pub seed: Option<PathBuf>,
    /// A directory of `*.sql` migrations, relative to `berm.toml`, applied in
    /// name order.
    pub migrations: Option<PathBuf>,
}

impl Project {
    /// The file's name.
    pub const FILE: &str = "berm.toml";

    /// `berm.toml` in `dir`.
    pub fn read(dir: &Path) -> Result<Self> {
        let path = dir.join(Self::FILE);
        let text = fs::read_to_string(&path)
            .with_context(|| format!("no {} in {}", Self::FILE, dir.display()))?;
        toml::from_str(&text).with_context(|| format!("cannot parse {}", path.display()))
    }

    /// The service a program deploys as, refused when it names none.
    pub fn name(&self) -> Result<&str> {
        self.name.as_deref().with_context(|| {
            format!(
                "a program's {} names its service: `name = \"…\"`",
                Self::FILE
            )
        })
    }

    /// What it reaches for once it runs. One keeping a database calls
    /// `berm.sql`.
    pub fn deps(&self) -> Deps {
        let mut syscalls = self.syscalls.clone();
        if self.database.migrations.is_some() {
            syscalls.entry("berm.sql".to_owned()).or_default();
        }
        Deps {
            programs: self.programs.clone(),
            syscalls,
            secrets: self.secrets.clone(),
        }
    }

    /// The most bytes a call's arguments, and a tool's result, may hold.
    pub fn buffer(&self) -> usize {
        self.buffer.unwrap_or(DEFAULT_BUFFER)
    }

    /// Refuse what only a program declares, for a kit's `berm.toml`.
    pub fn check_kit(&self) -> Result<()> {
        let program_only = [
            ("name", self.name.is_some()),
            ("image", self.image.is_some()),
            ("public", !self.public.is_empty()),
            ("crons", self.crons.is_some()),
            ("buffer", self.buffer.is_some()),
            ("readme", self.readme.is_some()),
            ("database.size", self.database.size.is_some()),
            ("database.seed", self.database.seed.is_some()),
        ];
        let declared: Vec<&str> = program_only
            .iter()
            .filter(|(_, set)| *set)
            .map(|(key, _)| *key)
            .collect();
        if !declared.is_empty() {
            bail!(
                "a kit's {} declares only secrets, programs, [syscalls] and [database] migrations, not {}",
                Self::FILE,
                declared.join(", ")
            );
        }
        Ok(())
    }
}

impl Database {
    /// What the service's database may hold, in bytes.
    pub fn size(&self) -> u64 {
        self.size.map_or(host::DATABASE_SIZE, |size| size.0)
    }
}
