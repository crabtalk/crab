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
