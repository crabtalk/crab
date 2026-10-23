//! Build a berm program's image.
//!
//! cargo builds the program's `cdylib` for [`TARGET`]. Its `.berm.abi` section
//! holds what the compiler wrote: a record per `#[tool]` and per `#[event]`.
//! The build appends a `.berm.program` section, from the program's package and
//! its `crab.toml`, and a `.berm.kits` section, from each kit it links: each
//! crate cargo compiled for [`TARGET`] in that build, other than the program,
//! with a `crab.toml` beside its `Cargo.toml`. The finished image is written to `<target directory>/berm/<lib name>.wasm`;
//! cargo's own `.wasm` is left as cargo wrote it.

use crate::{artifact::Artifact, metadata::Metadata};
use anyhow::{Context, Result, bail};
use berm_api::{
    Database, Image, KITS_SECTION, Kit, Manifest, Migration, PROGRAM_SECTION, ProgramRecord,
    project::Project,
};
use std::{
    env, fs,
    path::{Path, PathBuf},
    process,
};

pub use workspace::Workspace;

mod artifact;
mod metadata;
mod toolchain;
mod workspace;

/// What a program is built for.
pub const TARGET: &str = "wasm32-unknown-unknown";

/// The directory under cargo's target directory the finished images are
/// written to.
const OUT: &str = "berm";

/// Build the program `package` of the workspace `dir` is in, or, without
/// one, the program whose `Cargo.toml` is in `dir`, and answer the path of
/// its finished image. `file` names the project file beside the program's
/// `Cargo.toml` and each kit's.
///
/// One build at a time across processes, on a lock in the target directory.
pub fn build(dir: &Path, package: Option<&str>, file: &str) -> Result<PathBuf> {
    let cargo = env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned());
    let metadata = Metadata::read(&cargo, dir)?;
    let program = metadata.package(dir, package)?;
    let project = Project::read(&program.dir().join(file))?;
    project.name()?;
    if !program
        .targets
        .iter()
        .any(|target| target.crate_types.iter().any(|kind| kind == "cdylib"))
    {
        bail!(
            "{} builds no cdylib; a program's Cargo.toml has `[lib] crate-type = [\"cdylib\", \"rlib\"]`",
            program.name
        );
    }
    let record = ProgramRecord {
        name: program.name.clone(),
        version: program.version.clone(),
        description: program.description.clone().unwrap_or_default(),
        homepage: program.homepage.clone().unwrap_or_default(),
        repository: program.repository.clone().unwrap_or_default(),
        readme: match &project.readme {
            Some(readme) => {
                let path = program.dir().join(readme);
                fs::read_to_string(&path)
                    .with_context(|| format!("cannot read {}", path.display()))?
            }
            None => String::new(),
        },
        buffer: project.buffer(),
        deps: project.deps(),
        database: migrations(program.dir(), &project)?,
    };

    let out = metadata.target_directory.join(OUT);
    fs::create_dir_all(&out).with_context(|| format!("cannot create {}", out.display()))?;
    let lock = fs::File::create(out.join("build.lock"))?;
    lock.lock()?;

    toolchain::ensure(dir)?;
    let spec = format!("{}@{}", program.name, program.version);
    let artifacts = Artifact::build(
        &cargo,
        dir,
        &["--release", "--target", TARGET, "--lib", "-p", &spec],
    )
    .with_context(|| format!("building {}", program.name))?;
    let guest = metadata.target_directory.join(TARGET);
    let built = artifacts
        .iter()
        .filter(|artifact| artifact.manifest_path == program.manifest_path)
        .flat_map(|artifact| &artifact.filenames)
        .find(|file| {
            file.extension()
                .is_some_and(|extension| extension == "wasm")
        })
        .with_context(|| format!("cargo built no image of {}", program.name))?;
    let bytes = fs::read(built).with_context(|| format!("cannot read {}", built.display()))?;
    let kits = kits(&artifacts, &program.manifest_path, &guest, file)?;

    let bytes = Image(&bytes).with_section(PROGRAM_SECTION, &serde_json::to_vec(&record)?)?;
    let bytes = Image(&bytes).with_section(KITS_SECTION, &serde_json::to_vec(&kits)?)?;
    Manifest::from_image(&bytes).with_context(|| format!("{} is not a program", program.name))?;

    let file = built
        .file_name()
        .context("cargo built an image with no name")?;
    let image = out.join(file);
    let partial = out.join(format!("{}.{}", file.to_string_lossy(), process::id()));
    fs::write(&partial, &bytes).with_context(|| format!("cannot write {}", partial.display()))?;
    fs::rename(&partial, &image).with_context(|| format!("cannot write {}", image.display()))?;
    Ok(image)
}

/// The record of each kit among `artifacts` compiled into `guest`, the
/// target directory of [`TARGET`], other than the program at `program`, by
/// name. Refused when a kit is there at two versions.
fn kits(artifacts: &[Artifact], program: &Path, guest: &Path, file: &str) -> Result<Vec<Kit>> {
    let mut kits: Vec<(&Artifact, String, String)> = Vec::new();
    for artifact in artifacts {
        if artifact.manifest_path == program
            || !artifact
                .filenames
                .iter()
                .any(|file| file.starts_with(guest))
            || !artifact.dir().join(file).is_file()
            || kits
                .iter()
                .any(|(seen, ..)| seen.package_id == artifact.package_id)
        {
            continue;
        }
        let (name, version) = artifact.package()?;
        kits.push((artifact, name, version));
    }
    kits.sort_by(|a, b| a.1.cmp(&b.1));
    if let Some(twice) = kits.windows(2).find(|pair| pair[0].1 == pair[1].1) {
        bail!(
            "the program links the kit {} at two versions, {} and {}",
            twice[0].1,
            twice[0].2,
            twice[1].2
        );
    }
    kits.into_iter()
        .map(|(artifact, name, version)| {
            let project = Project::read(&artifact.dir().join(file))?;
            project
                .check_kit()
                .with_context(|| format!("the kit {name}"))?;
            Ok(Kit {
                database: migrations(artifact.dir(), &project)?,
                deps: project.deps(),
                id: project.id.clone(),
                formerly: project.formerly.clone(),
                name,
                version,
            })
        })
        .collect()
}

/// The `*.sql` files in the directory `[database] migrations` names, relative
/// to `dir`, by name.
fn migrations(dir: &Path, project: &Project) -> Result<Option<Database>> {
    let Some(migrations) = &project.database.migrations else {
        return Ok(None);
    };
    let path = dir.join(migrations);
    let unreadable = |e| anyhow::Error::new(e).context(format!("cannot read {}", path.display()));
    let mut read = Vec::new();
    for entry in fs::read_dir(&path).map_err(unreadable)? {
        let file = entry.map_err(unreadable)?.path();
        if file.extension().is_none_or(|extension| extension != "sql") {
            continue;
        }
        let name = file
            .file_name()
            .and_then(|name| name.to_str())
            .with_context(|| format!("{} is not a UTF-8 name", file.display()))?
            .to_owned();
        let sql =
            fs::read_to_string(&file).with_context(|| format!("cannot read {}", file.display()))?;
        read.push(Migration { name, sql });
    }
    read.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(Some(Database { migrations: read }))
}
