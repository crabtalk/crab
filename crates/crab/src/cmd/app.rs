//! `crab app` — the apps a website calls the account's services through.

use crate::utils;
use anyhow::{Result, bail};
use crabtalk_sdk::{
    account::app::{AppRequest, Issuer},
    client::Client,
};

pub fn ls(client: &Client, json: bool) -> Result<()> {
    let apps = utils::wait(client.apps())?;
    if json {
        return utils::json(&apps);
    }
    for app in &apps {
        println!("{}  {}", app.id, app.name);
        println!("  origins   {}", app.origins.join(", "));
        println!("  services  {}", app.services.join(", "));
        for issuer in &app.issuers {
            println!("  issuer    {} for {}", issuer.issuer, issuer.audience);
        }
        if app.anonymous {
            println!("  anonymous calls reach #[anonymous] tools");
        }
    }
    Ok(())
}

/// What an app is made or replaced with, `issuers` as `ISSUER=AUDIENCE`.
pub fn request(
    name: &str,
    origins: &[String],
    services: &[String],
    issuers: &[String],
    anonymous: bool,
) -> Result<AppRequest> {
    let issuers = issuers
        .iter()
        .map(|pair| match pair.split_once('=') {
            Some((issuer, audience)) => Ok(Issuer {
                issuer: issuer.to_owned(),
                audience: audience.to_owned(),
            }),
            None => bail!("{pair:?} is not ISSUER=AUDIENCE"),
        })
        .collect::<Result<_>>()?;
    Ok(AppRequest {
        name: name.to_owned(),
        origins: origins.to_vec(),
        issuers,
        services: services.to_vec(),
        anonymous,
    })
}

pub fn create(client: &Client, request: &AppRequest, json: bool) -> Result<()> {
    let app = utils::wait(client.create_app(request))?;
    if json {
        return utils::json(&app);
    }
    eprintln!("app {} made", app.id);
    Ok(())
}

pub fn update(client: &Client, id: &str, request: &AppRequest, json: bool) -> Result<()> {
    let app = utils::wait(client.update_app(id, request))?;
    if json {
        return utils::json(&app);
    }
    eprintln!("app {id} replaced");
    Ok(())
}

pub fn rm(client: &Client, id: &str, yes: bool) -> Result<()> {
    utils::confirm(yes, &format!("delete app {id}"))?;
    utils::wait(client.delete_app(id))?;
    eprintln!("deleted {id}");
    Ok(())
}
