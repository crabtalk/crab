use crate::{cmd::pull, deployment, runtime};
use anyhow::{Context, Result, bail};
use berm_api::reference::Reference;
use clap::Args;
use crabtalk_sdk::{
    client::Client,
    services::service::{DeployRequest, Overrides},
};
use std::collections::BTreeMap;

#[derive(Args)]
pub struct Deploy {
    pub image: String,
    #[arg(long = "as")]
    pub name: Option<String>,
    #[arg(long)]
    pub version: Option<String>,
    #[arg(long)]
    pub auto_upgrade: bool,
    #[arg(long, conflicts_with = "private")]
    pub public: bool,
    #[arg(long, conflicts_with = "public")]
    pub private: bool,
    #[arg(long = "price", value_name = "TOOL=MICROS")]
    pub prices: Vec<String>,
    #[arg(long)]
    pub force: bool,
}
impl Deploy {
    pub fn run(&self, client: &Client, cloud: &Client, yes: bool) -> Result<()> {
        if client.owner.is_none()
            && (self.public || self.private || !self.prices.is_empty() || self.auto_upgrade)
        {
            bail!(
                "--public, --private, --price and --auto-upgrade apply only on Cloud; switch to a Cloud context"
            );
        }
        let mut at: Reference = self.image.parse()?;
        if let Some(version) = &self.version {
            at.version = Some(version.parse()?);
        }
        let address = at.address();
        let selected = at.version.as_ref().map(ToString::to_string);
        let image = if client.owner.is_none() {
            if client.base == crate::cmd::dev::url("") {
                crate::cmd::dev::up()?;
            }
            match runtime::wait(client.published_image(&address, selected.as_deref())) {
                Ok(image) => image,
                Err(crabtalk_sdk::client::Error::Api { status: 404, .. }) => {
                    runtime::wait(cloud.published_image(&address, selected.as_deref()))?
                }
                Err(error) => return Err(error.into()),
            }
        } else {
            runtime::wait(cloud.published_image(&address, selected.as_deref()))?
        };
        let mut prices = BTreeMap::new();
        for entry in &self.prices {
            let (tool, value) = entry.split_once('=').context("--price takes TOOL=MICROS")?;
            if prices.insert(tool.to_owned(), value.parse()?).is_some() {
                bail!("--price repeats {tool}");
            }
        }
        let overrides = Overrides {
            public: if self.public {
                Some(true)
            } else if self.private {
                Some(false)
            } else {
                None
            },
            prices,
        };
        let name = self.name.clone().unwrap_or(at.name.clone());
        let own = client
            .owner
            .as_ref()
            .is_some_and(|owner| owner.name() == image.owner || owner.name() == at.owner);
        if own && !overrides.is_empty() {
            bail!("change visibility and prices in your own image's code");
        }
        let previous = if client.owner.is_none() {
            Ok(Vec::new())
        } else {
            runtime::wait(client.deploys(&name))
        };
        let mut effective_overrides = match previous {
            Ok(deploys) => deploys
                .into_iter()
                .find(|d| d.outcome == crabtalk_sdk::services::service::DeployOutcome::Live)
                .map(|d| d.overrides)
                .unwrap_or_default(),
            Err(crabtalk_sdk::client::Error::Api { status: 404, .. }) => Overrides::default(),
            Err(error) => return Err(error.into()),
        };
        if own {
            effective_overrides = Overrides::default();
        }
        effective_overrides.extend(&overrides);
        let mut effective = image.manifest.clone();
        effective_overrides
            .apply(&mut effective)
            .map_err(anyhow::Error::msg)?;
        if client.owner.is_some() {
            eprintln!(
                "{address}:{} — {}",
                effective.version,
                if effective.public {
                    "public"
                } else {
                    "private"
                }
            );
            for tool in effective.tools.iter().filter(|t| t.public) {
                eprintln!(
                    "  {}: {} USD micros",
                    tool.name,
                    tool.price.unwrap_or_default()
                );
            }
        } else {
            eprintln!("{address}:{}", effective.version);
        }
        let reference = if client.owner.is_none() {
            pull::store(
                client,
                cloud,
                &address,
                &image.manifest.version,
                &image.digest,
            )?
        } else {
            format!("{address}:{}", image.manifest.version)
        };
        let request = DeployRequest {
            reference,
            overrides,
            auto_upgrade: self.auto_upgrade.then_some(true),
            force: self.force,
            ..Default::default()
        };
        deployment::submit(
            client,
            BTreeMap::from([(name, request)]),
            BTreeMap::new(),
            yes,
        )
    }
}
