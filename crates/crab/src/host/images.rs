//! Uploaded images, one file per digest under `{root}/images`.

use anyhow::{Context, Result};
use berm_host::Images;
use std::path::{Path, PathBuf};
use tokio::fs;

/// What a digest opens with.
const SHA256: &str = "sha256:";

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

    /// Keep `image`, answering with its digest.
    pub async fn put(&self, image: &[u8]) -> Result<String> {
        let hex = berm::digest(image);
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
        Ok(format!("{SHA256}{hex}"))
    }
}

impl Images for Stored {
    async fn pull(&self, _: &str, digest: &str) -> Result<Vec<u8>> {
        let hex = digest
            .strip_prefix(SHA256)
            .with_context(|| format!("{digest:?} is not a sha256 digest"))?;
        fs::read(self.dir.join(hex))
            .await
            .with_context(|| format!("no image {digest} is kept"))
    }
}
