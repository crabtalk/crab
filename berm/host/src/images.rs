//! Uploaded images, one file per digest under `{root}/images`.

use crate::Images;
use anyhow::{Context, Result};
use berm_api::digest;
use std::path::{Path, PathBuf};
use tokio::fs;

#[derive(Clone)]
pub struct Stored {
    dir: PathBuf,
}

impl Stored {
    pub fn open(root: &Path) -> Self {
        Self {
            dir: root.join("images"),
        }
    }

    pub async fn bind(
        &self,
        reference: &berm_api::reference::Reference,
        digest: &str,
    ) -> Result<()> {
        anyhow::ensure!(self.holds(digest).await, "no image {digest} is kept");
        let version = reference
            .version
            .as_ref()
            .context("a stored image reference needs a version")?;
        let dir = self
            .dir
            .join("references")
            .join(&reference.owner)
            .join(&reference.name);
        fs::create_dir_all(&dir).await?;
        let path = dir.join(version.to_string());
        let staging = dir.join(uuid::Uuid::new_v4().to_string());
        fs::write(&staging, digest).await?;
        let linked = fs::hard_link(&staging, &path).await;
        fs::remove_file(&staging).await?;
        match linked {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                anyhow::ensure!(
                    fs::read_to_string(&path).await? == digest,
                    "this local image version already identifies different bytes"
                );
            }
            Err(error) => return Err(error.into()),
        }
        Ok(())
    }

    pub async fn resolve(
        &self,
        reference: &berm_api::reference::Reference,
    ) -> Result<Option<String>> {
        let dir = self
            .dir
            .join("references")
            .join(&reference.owner)
            .join(&reference.name);
        let mut entries = match fs::read_dir(&dir).await {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e.into()),
        };
        let mut versions = Vec::new();
        while let Some(entry) = entries.next_entry().await? {
            if let Ok(version) = entry
                .file_name()
                .to_string_lossy()
                .parse::<semver::Version>()
                && reference
                    .version
                    .as_ref()
                    .is_none_or(|selected| selected == &version)
            {
                versions.push(version);
            }
        }
        let Some(version) = versions
            .into_iter()
            .max_by_key(|v| (v.pre.is_empty(), v.clone()))
        else {
            return Ok(None);
        };
        let digest = fs::read_to_string(dir.join(version.to_string())).await?;
        Ok(self.holds(&digest).await.then_some(digest))
    }

    /// Whether `reference` names an uploaded image: one deployed by its
    /// digest alone.
    pub fn uploaded(reference: &str, digest: &str) -> bool {
        reference == digest
    }

    /// Whether the image `digest` names is kept.
    pub async fn holds(&self, digest: &str) -> bool {
        match digest.strip_prefix(digest::PREFIX) {
            Some(hex) => fs::try_exists(self.dir.join(hex)).await.unwrap_or(false),
            None => false,
        }
    }

    /// Keep `image`, answering with its digest.
    pub async fn put(&self, image: &[u8]) -> Result<String> {
        let hex = digest::hex(image);
        let path = self.dir.join(&hex);
        if !fs::try_exists(&path).await.unwrap_or(false) {
            fs::create_dir_all(&self.dir)
                .await
                .context("failed to open the images directory")?;
            let staging = path.with_extension("writing");
            fs::write(&staging, image)
                .await
                .context("failed to stage an image")?;
            fs::rename(&staging, &path)
                .await
                .context("failed to keep an image")?;
        }
        Ok(digest::prefixed(&hex))
    }
}

impl Images for Stored {
    async fn pull(&self, _: &str, digest: &str) -> Result<Vec<u8>> {
        let hex = digest
            .strip_prefix(digest::PREFIX)
            .with_context(|| format!("{digest:?} is not a sha256 digest"))?;
        fs::read(self.dir.join(hex))
            .await
            .with_context(|| format!("no image {digest} is kept"))
    }
}
