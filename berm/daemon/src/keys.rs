//! The key each program's ID tokens are signed with, and the issuer they name.
//!
//! One Ed25519 key per program, generated on first use and kept as PKCS#8 in
//! `{root}/keys/{program}.p8`, readable by its owner alone on Unix. Only the
//! public half is served. A program's issuer is `{url}/issuers/{program}`.

use anyhow::{Context, Result};
use berm_host::{Issuer, Key};
use std::{
    collections::HashMap,
    io::ErrorKind,
    path::{Path, PathBuf},
    sync::Arc,
};
use tokio::{fs, io::AsyncWriteExt, sync::Mutex};

/// Where the keys live, and the ones read so far.
#[derive(Clone)]
pub struct Keys {
    dir: PathBuf,
    /// This service's base URL, without a trailing `/`.
    url: Arc<str>,
    held: Arc<Mutex<HashMap<String, Arc<Key>>>>,
}

impl Keys {
    /// The keys under `root`, for a service reached at `url`.
    pub fn open(root: &Path, url: &str) -> Self {
        Self {
            dir: root.join("keys"),
            url: url.trim_end_matches('/').into(),
            held: Arc::default(),
        }
    }

    /// `program`'s key, generated and kept if it has none.
    pub async fn key(&self, program: &str) -> Result<Arc<Key>> {
        let mut held = self.held.lock().await;
        if let Some(key) = held.get(program) {
            return Ok(key.clone());
        }
        let path = self.dir.join(format!("{program}.p8"));
        let key = match fs::read(&path).await {
            Ok(document) => Key::from_pkcs8(&document)
                .with_context(|| format!("{} is not a key", path.display()))?,
            Err(error) if error.kind() == ErrorKind::NotFound => self.keep(&path).await?,
            Err(error) => {
                return Err(error).with_context(|| format!("failed to read {}", path.display()));
            }
        };
        let key = Arc::new(key);
        held.insert(program.to_owned(), key.clone());
        Ok(key)
    }

    /// A fresh key, written to `path`.
    async fn keep(&self, path: &Path) -> Result<Key> {
        let (key, document) = Key::generate()?;
        fs::create_dir_all(&self.dir)
            .await
            .context("failed to open the keys directory")?;
        let staging = path.with_extension("writing");
        let mut options = fs::OpenOptions::new();
        options.write(true).create(true).truncate(true);
        #[cfg(unix)]
        options.mode(0o600);
        let mut file = options
            .open(&staging)
            .await
            .context("failed to stage a key")?;
        file.write_all(&document)
            .await
            .context("failed to stage a key")?;
        file.sync_all().await.context("failed to stage a key")?;
        fs::rename(&staging, path)
            .await
            .context("failed to keep a key")?;
        Ok(key)
    }
}

impl Issuer for Keys {
    async fn issuer(&self, program: &str) -> Result<String> {
        Ok(format!("{}/issuers/{program}", self.url))
    }

    async fn kid(&self, program: &str) -> Result<String> {
        Ok(self.key(program).await?.kid().to_owned())
    }

    async fn sign(&self, program: &str, message: &[u8]) -> Result<Vec<u8>> {
        Ok(self.key(program).await?.sign(message))
    }
}
