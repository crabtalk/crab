//! Where `crab secret fill` sets secrets: the workspace's `secrets.toml`, or a
//! Cloud account.

use crate::utils;
use anyhow::{Context, Result};
use crabtalk_sdk::client::Client;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::{
    collections::BTreeMap,
    fmt, fs, io,
    path::{Path, PathBuf},
    process::Command,
};

/// Where `crab secret fill` sets secrets.
pub enum Target {
    /// The workspace's `secrets.toml`, which the local host reads.
    Workspace(PathBuf),
    /// The account a Cloud context acts for.
    Cloud(String),
}

impl fmt::Display for Target {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Target::Workspace(path) => write!(f, "development, this machine: {}", path.display()),
            Target::Cloud(account) => write!(f, "Cloud, the account {account}"),
        }
    }
}

impl Target {
    /// The secrets set now, by name, each beside its value when it can be
    /// read back.
    pub fn stored(&self, client: &Client, name: &str) -> Result<BTreeMap<String, Option<String>>> {
        match self {
            Target::Workspace(path) => Ok(table(path)?
                .iter()
                .filter_map(|(key, item)| Some((key.to_owned(), Some(item.as_str()?.to_owned()))))
                .collect()),
            Target::Cloud(_) => Ok(utils::wait(client.secrets(name))?
                .into_iter()
                .map(|secret| (secret.name, None))
                .collect()),
        }
    }

    /// Set each of `values`, replacing any of them in `stored`.
    pub fn save(
        &self,
        client: &Client,
        name: &str,
        values: &[(String, String)],
        stored: &BTreeMap<String, Option<String>>,
    ) -> Result<()> {
        match self {
            Target::Workspace(path) => {
                let mut table = table(path)?;
                for (secret, value) in values {
                    table[secret.as_str()] = toml_edit::value(value.as_str());
                }
                ignored(path)?;
                let written = path.with_extension("toml.partial");
                fs::write(&written, table.to_string())
                    .with_context(|| format!("cannot write {}", written.display()))?;
                #[cfg(unix)]
                fs::set_permissions(&written, fs::Permissions::from_mode(0o600))?;
                fs::rename(&written, path)
                    .with_context(|| format!("cannot write {}", path.display()))?;
                Ok(())
            }
            Target::Cloud(_) => {
                for (secret, value) in values {
                    if stored.contains_key(secret) {
                        utils::wait(client.delete_secret(name, secret))?;
                    }
                    utils::wait(client.put_secret(name, secret, value))?;
                }
                Ok(())
            }
        }
    }
}

/// The `secrets.toml` at `path`, empty while it is absent.
fn table(path: &Path) -> Result<toml_edit::DocumentMut> {
    match fs::read_to_string(path) {
        Ok(text) => text
            .parse()
            .with_context(|| format!("{} is not valid TOML", path.display())),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Default::default()),
        Err(error) => Err(error).with_context(|| format!("cannot read {}", path.display())),
    }
}

/// Add `path`'s name to the `.gitignore` beside it when git, in the repository
/// it is in, does not ignore it.
fn ignored(path: &Path) -> Result<()> {
    let (Some(dir), Some(file)) = (path.parent(), path.file_name()) else {
        return Ok(());
    };
    let checked = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["check-ignore", "-q"])
        .arg(file)
        .status();
    // 1 is a file git would track; anything else is ignored, or no repository.
    if !checked.is_ok_and(|status| status.code() == Some(1)) {
        return Ok(());
    }
    let gitignore = dir.join(".gitignore");
    let mut text = fs::read_to_string(&gitignore).unwrap_or_default();
    if !text.is_empty() && !text.ends_with('\n') {
        text.push('\n');
    }
    text.push_str(&format!("{}\n", file.to_string_lossy()));
    fs::write(&gitignore, text).with_context(|| format!("cannot write {}", gitignore.display()))?;
    eprintln!(
        "added {} to {}",
        file.to_string_lossy(),
        gitignore.display()
    );
    Ok(())
}
