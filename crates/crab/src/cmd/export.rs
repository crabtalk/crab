use crate::runtime;
use anyhow::{Context, Result};
use crabtalk_sdk::client::Client;
use std::path::{Path, PathBuf};
use tempfile::NamedTempFile;
use tokio::io::AsyncWriteExt;

pub fn run(client: &Client, name: &str, out: Option<&Path>) -> Result<()> {
    let default = PathBuf::from(format!(
        "{}.sqlite",
        name.rsplit('/').next().unwrap_or(name)
    ));
    let path = out.unwrap_or(&default);
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let temporary =
        NamedTempFile::new_in(parent).with_context(|| format!("creating {}", path.display()))?;
    runtime::wait(async {
        let mut response = client.export(name).await?;
        let mut file = tokio::fs::File::from_std(temporary.reopen()?);
        while let Some(chunk) = response.chunk().await? {
            file.write_all(&chunk).await?;
        }
        file.flush().await?;
        file.sync_all().await?;
        Ok::<_, anyhow::Error>(())
    })?;
    temporary.persist_noclobber(path).with_context(|| {
        format!(
            "writing {}; choose another path with -o if it exists",
            path.display()
        )
    })?;
    println!("{}", path.display());
    Ok(())
}
