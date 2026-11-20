use crate::{cmd::dev, deployment, runtime};
use anyhow::{Result, ensure};
use berm_api::{digest, reference::Reference};
use crabtalk_sdk::client::Client;

pub fn run(cloud: &Client, name: &str) -> Result<()> {
    let at: Reference = name.parse()?;
    let image = runtime::wait(cloud.published_image(
        &at.address(),
        at.version.as_ref().map(ToString::to_string).as_deref(),
    ))?;
    dev::up()?;
    let local = Client::new(dev::url(""));
    store(
        &local,
        cloud,
        &at.address(),
        &image.manifest.version,
        &image.digest,
    )?;
    println!(
        "{}:{} ({})",
        at.address(),
        image.manifest.version,
        image.digest
    );
    Ok(())
}

pub fn store(
    local: &Client,
    cloud: &Client,
    address: &str,
    version: &str,
    expected: &str,
) -> Result<String> {
    if local.base == dev::url("") {
        dev::up()?;
    }
    match runtime::wait(local.published_image(address, Some(version))) {
        Ok(image) => {
            ensure!(
                image.digest == expected,
                "this local image version already identifies different bytes"
            );
            return Ok(image.digest);
        }
        Err(crabtalk_sdk::client::Error::Api { status: 404, .. }) => {}
        Err(error) => return Err(error.into()),
    }
    let bytes = runtime::wait(cloud.download_image(address, Some(version)))?;
    ensure!(
        digest::prefixed(&digest::hex(&bytes)) == expected,
        "downloaded image digest does not match its manifest"
    );
    let digest = deployment::send(local, address.rsplit('/').next().unwrap_or(address), bytes)?;
    runtime::wait(local.publish_image(address, &digest))?;
    Ok(digest)
}
