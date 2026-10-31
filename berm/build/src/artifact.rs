//! What `cargo build` reports compiling, one `compiler-artifact` message per
//! crate, fresh or not.

use crate::metadata::ManifestPath;
use anyhow::{Context, Result, bail};
use serde::Deserialize;
use serde_json::Value;
use std::{
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

/// One crate a build compiled, or found fresh.
#[derive(Deserialize)]
pub struct Artifact {
    pub package_id: String,
    pub manifest_path: ManifestPath,
    pub filenames: Vec<PathBuf>,
}

impl Artifact {
    /// Run `cargo build` with `args` in `dir` and answer each crate it
    /// compiled or found fresh, host crates included. Diagnostics go to
    /// stderr as cargo renders them.
    pub fn build(cargo: &str, dir: &Path, args: &[&str]) -> Result<Vec<Self>> {
        let mut child = Command::new(cargo)
            .arg("build")
            .args(args)
            .args(["--message-format", "json-render-diagnostics"])
            .current_dir(dir)
            .stdout(Stdio::piped())
            .spawn()
            .context("cannot run cargo")?;
        let mut stdout = String::new();
        child
            .stdout
            .take()
            .context("cargo has no stdout")?
            .read_to_string(&mut stdout)
            .context("cannot read cargo's output")?;
        if !child.wait().context("cannot run cargo")?.success() {
            bail!("cargo build failed");
        }
        stdout
            .lines()
            .filter_map(|line| serde_json::from_str::<Value>(line).ok())
            .filter(|message| message["reason"] == "compiler-artifact")
            .map(|message| {
                serde_json::from_value(message).context("cargo reported an artifact it never does")
            })
            .collect()
    }

    /// Its package's name and version, read off its package ID: a spec such
    /// as `path+file:///kits/auth#crabkit-auth@0.1.1`, `path+file:///auth#0.1.1`,
    /// or, before cargo 1.77, `auth 0.1.1 (path+file:///auth)`.
    pub fn package(&self) -> Result<(String, String)> {
        let id = &self.package_id;
        let unread = || format!("cannot read the package ID {id}");
        if let Some((name, rest)) = id.split_once(' ') {
            let version = rest.split_once(' ').map_or(rest, |(version, _)| version);
            return Ok((name.to_owned(), version.to_owned()));
        }
        let (url, fragment) = id.rsplit_once('#').with_context(unread)?;
        Ok(match fragment.rsplit_once('@') {
            Some((name, version)) => (name.to_owned(), version.to_owned()),
            None => {
                let name = url
                    .trim_end_matches('/')
                    .rsplit('/')
                    .next()
                    .with_context(unread)?;
                (name.to_owned(), fragment.to_owned())
            }
        })
    }
}
