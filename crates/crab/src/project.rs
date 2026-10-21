//! A program's project: the `berm.toml` beside its `Cargo.toml`, and the image
//! cargo builds from it.

use anyhow::{Context, Result, bail};
use berm_api::host;
use bytesize::ByteSize;
use serde::Deserialize;
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    process::Command,
};
use toml_edit::{DocumentMut, Item, Table, value};

/// The file `crab deploy` reads, in the directory it runs in.
pub const FILE: &str = "berm.toml";

/// What a program is built for.
pub const TARGET: &str = "wasm32-unknown-unknown";

/// `berm.toml`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Project {
    /// The service this program deploys as.
    pub name: String,
    /// The registry repository Cloud pulls the image from, without a tag.
    #[serde(default)]
    pub image: Option<String>,
    /// The tools this program may make public.
    #[serde(default)]
    pub public: Vec<String>,
    /// Each cron tool's expression, five fields in UTC, such as
    /// `"*/5 * * * *"`. Absent, they are its owners' to set on the host.
    #[serde(default)]
    pub crons: Option<BTreeMap<String, String>>,
    #[serde(default)]
    pub database: Database,
}

/// `[database]` of `berm.toml`.
#[derive(Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Database {
    /// What the service's database may hold, such as `"2GiB"`, from
    /// [`host::MIN_DATABASE_SIZE`] to [`host::MAX_DATABASE_SIZE`] bytes.
    pub size: ByteSize,
    /// A SQLite database, relative to `berm.toml`, a service on Cloud with no
    /// database yet starts from, such as one a bermd filled.
    pub seed: Option<PathBuf>,
}

impl Default for Database {
    fn default() -> Self {
        Self {
            size: ByteSize(host::DATABASE_SIZE),
            seed: None,
        }
    }
}

impl Project {
    /// `berm.toml` in `dir`.
    pub fn read(dir: &Path) -> Result<Self> {
        let path = dir.join(FILE);
        let text =
            fs::read_to_string(&path).with_context(|| format!("no {FILE} in {}", dir.display()))?;
        toml::from_str(&text).with_context(|| format!("cannot parse {}", path.display()))
    }
}

/// Write `crons` as `[crons]` of the `berm.toml` in `dir`, in place of the one
/// there; the rest of the file is kept as written.
pub fn write_crons(dir: &Path, crons: &BTreeMap<String, String>) -> Result<()> {
    let path = dir.join(FILE);
    let text =
        fs::read_to_string(&path).with_context(|| format!("no {FILE} in {}", dir.display()))?;
    let mut file: DocumentMut = text
        .parse()
        .with_context(|| format!("cannot parse {}", path.display()))?;
    let mut table = Table::new();
    for (tool, cron) in crons {
        table.insert(tool, value(cron));
    }
    file.insert("crons", Item::Table(table));
    fs::write(&path, file.to_string()).with_context(|| format!("cannot write {}", path.display()))
}

/// Build the one binary of the package whose manifest is `dir/Cargo.toml` for
/// [`TARGET`], answering with the image's bytes.
pub fn build(dir: &Path) -> Result<Vec<u8>> {
    #[derive(Deserialize)]
    struct Metadata {
        packages: Vec<Package>,
        target_directory: PathBuf,
    }
    #[derive(Deserialize)]
    struct Package {
        name: String,
        manifest_path: PathBuf,
        targets: Vec<Target>,
    }
    #[derive(Deserialize)]
    struct Target {
        name: String,
        kind: Vec<String>,
    }

    let manifest = dir
        .join("Cargo.toml")
        .canonicalize()
        .with_context(|| format!("no Cargo.toml in {}", dir.display()))?;
    let output = Command::new("cargo")
        .args(["metadata", "--no-deps", "--format-version", "1"])
        .current_dir(dir)
        .output()
        .context("cannot run cargo")?;
    if !output.status.success() {
        bail!("{}", String::from_utf8_lossy(&output.stderr).trim());
    }
    let metadata: Metadata = serde_json::from_slice(&output.stdout)?;
    let package = metadata
        .packages
        .into_iter()
        .find(|package| package.manifest_path == manifest)
        .with_context(|| format!("{} is not a package", manifest.display()))?;
    let bins: Vec<String> = package
        .targets
        .into_iter()
        .filter(|target| target.kind.iter().any(|kind| kind == "bin"))
        .map(|target| target.name)
        .collect();
    let [bin] = bins.as_slice() else {
        bail!(
            "{} has {} binaries; a program is built from exactly one",
            package.name,
            bins.len()
        );
    };

    let status = Command::new("cargo")
        .args([
            "build",
            "--release",
            "--target",
            TARGET,
            "-p",
            &package.name,
        ])
        .args(["--bin", bin])
        .current_dir(dir)
        .status()
        .context("cannot run cargo")?;
    if !status.success() {
        bail!("cargo build failed");
    }
    let image = metadata
        .target_directory
        .join(TARGET)
        .join("release")
        .join(format!("{bin}.wasm"));
    fs::read(&image).with_context(|| format!("cannot read {}", image.display()))
}
