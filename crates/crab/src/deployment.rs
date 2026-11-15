use crate::{
    cmd::{inspect, request},
    runtime,
};
use anyhow::{Result, bail};
use berm_api::{Deployed, Manifest, digest};
use berm_oci::{Access, Reference, Registry};
use crabtalk_sdk::{
    account::request::RequestStatus,
    client::Client,
    services::service::{DeployRequest, DeployResult, DeploySetRequest, UploadRequest},
};
use std::{collections::BTreeMap, str::FromStr};

pub(crate) fn submit(
    client: &Client,
    mut services: BTreeMap<String, DeployRequest>,
    copy_values: BTreeMap<String, BTreeMap<String, String>>,
    yes: bool,
) -> Result<()> {
    if let Some(owner) = &client.owner
        && !yes
    {
        return approved(client, owner.name(), services, copy_values);
    }
    let deployed = match services.pop_first() {
        Some((name, request)) if services.is_empty() => {
            let deployed = runtime::wait(client.deploy(&name, &request))?;
            BTreeMap::from([(name, DeployResult::from(deployed))])
        }
        first => {
            services.extend(first);
            runtime::wait(client.deploy_set(&DeploySetRequest {
                services,
                partial: !yes,
            }))?
        }
    };
    report_results(client, &deployed)
}

pub(crate) fn report_results(
    client: &Client,
    results: &BTreeMap<String, DeployResult>,
) -> Result<()> {
    for (name, deployed) in results {
        match deployed {
            DeployResult::Live(deployed) => report(client, name, deployed)?,
            DeployResult::Published { digest, published } => {
                println!(
                    "{name}: template {} ({digest})",
                    if *published {
                        "published"
                    } else {
                        "unpublished"
                    }
                );
            }
            DeployResult::Failed { failure, explain } => {
                eprintln!("{name}: {failure} {}", explain.as_deref().unwrap_or(""))
            }
        }
    }
    if results
        .values()
        .any(|one| matches!(one, DeployResult::Failed { .. }))
    {
        bail!("some services failed");
    }
    Ok(())
}

pub(crate) fn approved(
    client: &Client,
    account: &str,
    services: BTreeMap<String, DeployRequest>,
    copy: BTreeMap<String, BTreeMap<String, String>>,
) -> Result<()> {
    let asked = runtime::wait(client.ask_deploy_for(account, services, copy, ""))?;
    let minutes = (asked.expires_at - asked.created_at) / 60;
    eprintln!(
        "a person approves this request at the link, signed in, within {minutes} minutes; automation requires a key with deployments:write scope"
    );
    println!("{}", asked.url);
    let settled = request::settled(client, &asked.code, (minutes * 60) as u64)?;
    match settled.status {
        RequestStatus::Pending => bail!("nobody approved the request in {minutes} minutes"),
        RequestStatus::Deploying | RequestStatus::Running => {
            bail!("the request is still running; check its page for results")
        }
        RequestStatus::Declined => bail!("the person declined the request"),
        RequestStatus::Expired => {
            bail!("the request expired; check its page for results before submitting again")
        }
        RequestStatus::Completed | RequestStatus::Failed => {}
    }
    if let Some(failure) = &settled.failure {
        match &settled.explain {
            Some(code) => bail!("the approved request failed [{code}]: {failure}"),
            None => bail!("the approved request failed: {failure}"),
        }
    }
    let mut results = settled.results;
    for (name, deployed) in settled.deployed {
        results.entry(name).or_insert_with(|| deployed.into());
    }
    report_results(client, &results)
}

pub(crate) fn reference(
    client: &Client,
    name: &str,
    repository: &str,
    registry: Option<&str>,
    bytes: Vec<u8>,
) -> Result<String> {
    match registry {
        Some(registry) => {
            let version = Manifest::try_from(bytes.as_slice())?.version;
            let reference = Reference::from_str(&format!("{registry}/{repository}:{version}"))?;
            let digest =
                Registry::open(&reference, Access::Write)?.push(&reference.reference, &bytes)?;
            Ok(format!(
                "{}/{}@{digest}",
                reference.registry, reference.repository
            ))
        }
        None => send(client, name, bytes),
    }
}

/// Show what `name` serves now that `deployed` installed it, how it breaks
/// callers, and the secrets it needs that are not set.
fn report(client: &Client, name: &str, deployed: &Deployed) -> Result<()> {
    // The host compiled it to answer, so what comes back is what it will
    // serve.
    let (running, terms, set) = runtime::wait(async {
        tokio::try_join!(
            client.runtime(name),
            client.terms(name),
            client.secrets(name)
        )
    })?;
    inspect::show(&deployed.service, Some(&running), &terms, None);
    for broken in &deployed.breaks {
        println!("  breaks  {broken}");
    }
    let unset: Vec<&String> = running
        .deps
        .secrets
        .iter()
        .filter(|secret| !set.iter().any(|one| one.name == **secret && one.sent()))
        .collect();
    for secret in &unset {
        println!("  unset   secret {secret}");
    }
    Ok(())
}

/// Upload `image` for `name`'s deploy unless the host holds it already,
/// answering the digest the deploy names it by.
fn send(client: &Client, name: &str, image: Vec<u8>) -> Result<String> {
    let sha256 = digest::hex(&image);
    let size = image.len() as u64;
    let request = UploadRequest {
        sha256: sha256.clone(),
        size,
    };
    if let Some(url) = runtime::wait(client.image(name, &request))?.upload {
        eprintln!("uploading the image, {size} bytes");
        runtime::wait(client.upload(&url, image, size))?;
    }
    Ok(digest::prefixed(&sha256))
}
