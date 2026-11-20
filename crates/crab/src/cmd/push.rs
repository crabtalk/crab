use crate::{project, runtime};
use anyhow::{Context, Result, bail};
use clap::Args;
use crabtalk_sdk::{client::Client, services::service::UploadRequest};
use std::{collections::BTreeSet, fs, path::PathBuf};
#[derive(Args)]
pub struct Push {
    #[arg(default_value = ".")]
    pub dir: PathBuf,
    #[arg(short = 'p', long = "package")]
    pub packages: Vec<String>,
}
impl Push {
    pub fn run(&self, client: &Client) -> Result<()> {
        if client.owner.is_none() {
            bail!("pushing an image requires a Cloud context");
        }
        let selection = project::Selection::read(&self.dir, &self.packages, true)?;
        if selection.services.is_empty() {
            bail!("no service at or below {}", self.dir.display());
        }
        let mut names = BTreeSet::new();
        let mut pushed = false;
        for service in selection.services {
            let image = service
                .project
                .image
                .as_ref()
                .context("pushing requires an [image] name")?;
            if !image.publish {
                if !self.packages.is_empty() {
                    bail!("{} sets image.publish = false", image.name.address());
                }
                eprintln!("skipping {}: image.publish = false", image.name.address());
                continue;
            }
            pushed = true;
            let address = image.name.address();
            if !names.insert(address.clone()) {
                bail!("more than one service publishes {address}");
            }
            let built = berm_build::build(&service.dir, None)?;
            let bytes = fs::read(built)?;
            let sha256 = berm_api::digest::hex(&bytes);
            let size = bytes.len() as u64;
            if let Some(url) = runtime::wait(client.image_upload(
                &address,
                &UploadRequest {
                    sha256: sha256.clone(),
                    size,
                },
            ))?
            .upload
            {
                runtime::wait(client.upload(&url, bytes, size))?;
            }
            runtime::wait(client.publish_image(&address, &berm_api::digest::prefixed(&sha256)))?;
            println!("{address}:{} ({sha256})", service.source_version()?);
        }
        if !pushed {
            bail!("no service at or below {} publishes", self.dir.display());
        }
        Ok(())
    }
}
