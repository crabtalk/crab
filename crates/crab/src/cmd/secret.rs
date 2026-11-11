//! `crab secret` — a service's secrets.

use crate::{cmd::request, fill::Form, runtime, terminal};
use anyhow::{Context, Result, bail};
use crabtalk_sdk::{
    account::secret::{Create, Grant, Setup},
    client::Client,
};
use std::{
    collections::BTreeMap,
    io::{self, IsTerminal, Read},
};

/// Set `name`'s secret `secret` to the value on stdin, its trailing newline
/// dropped, sent only to its manifest destinations.
pub fn put(client: &Client, name: &str, secret: &str) -> Result<()> {
    let value = value()?;
    runtime::wait(client.put_secret(name, secret, &value))?;
    eprintln!("set {name} {secret} for its manifest destinations");
    Ok(())
}

fn value() -> Result<String> {
    let mut value = String::new();
    io::stdin()
        .read_to_string(&mut value)
        .context("reading the secret value from stdin")?;
    let value = value.trim_end_matches(['\n', '\r']);
    if value.is_empty() {
        bail!("Supply the secret value on stdin");
    }
    Ok(value.to_owned())
}

pub fn create_resource(client: &Client, name: &str) -> Result<()> {
    let resource = runtime::wait(client.create_secret_resource(&Create {
        name: name.into(),
        value: value()?,
    }))?;
    println!("{}", resource.id);
    Ok(())
}
pub fn resources(client: &Client) -> Result<()> {
    terminal::json(&runtime::wait(client.secret_resources())?)
}
pub fn rotate_resource(client: &Client, id: &str) -> Result<()> {
    runtime::wait(client.rotate_secret_resource(id, &value()?))?;
    Ok(())
}
pub fn delete_resource(client: &Client, id: &str, yes: bool) -> Result<()> {
    terminal::confirm(
        yes,
        &format!("delete secret resource {id} and all its grants"),
    )?;
    runtime::wait(client.delete_secret_resource(id))?;
    Ok(())
}
pub fn grant_resource(client: &Client, id: &str, service: &str, hosts: &[String]) -> Result<()> {
    runtime::wait(client.grant_secret_resource(
        id,
        &Grant {
            service: service.into(),
            hosts: hosts.to_vec(),
        },
    ))?;
    Ok(())
}
pub fn revoke_resource(client: &Client, id: &str, service: &str, yes: bool) -> Result<()> {
    terminal::confirm(
        yes,
        &format!("revoke {service}'s grant to secret resource {id}"),
    )?;
    runtime::wait(client.revoke_secret_resource(id, service))?;
    Ok(())
}
pub fn request_resource(
    client: &Client,
    service: &str,
    name: &str,
    hosts: &[String],
    id: Option<&str>,
    context: &str,
    json: bool,
) -> Result<()> {
    request::asked(
        &runtime::wait(client.ask_secret_resource(
            service,
            Setup {
                name: name.into(),
                hosts: hosts.to_vec(),
                id: id.map(str::to_owned),
            },
            context,
        ))?,
        json,
    )
}

/// Print `name`'s secrets, by name, each beside the hosts it is sent to.
pub fn ls(client: &Client, name: &str, json: bool) -> Result<()> {
    let secrets = runtime::wait(client.secrets(name))?;
    if json {
        return terminal::json(&secrets);
    }
    for secret in &secrets {
        match secret.sent() {
            true => println!("{}  {}", secret.name, secret.hosts.join(", ")),
            false => println!("{}  sent nowhere", secret.name),
        }
    }
    Ok(())
}

/// Revoke `name`'s secret `secret`.
pub fn rm(client: &Client, name: &str, secret: &str, yes: bool) -> Result<()> {
    terminal::confirm(yes, &format!("revoke {name} {secret}"))?;
    runtime::wait(client.delete_secret(name, secret))?;
    eprintln!("revoked {name} {secret}");
    Ok(())
}

/// Ask a person to set `name`'s unset secrets, or its credential
/// `credential`, with `copies` the values its credentials have them enter at
/// the provider and `context` shown to them: print its page, and how to wait
/// for it.
pub fn request(
    client: &Client,
    name: &str,
    credential: Option<&str>,
    copies: &[(String, String)],
    context: &str,
    json: bool,
) -> Result<()> {
    let mut copy = BTreeMap::new();
    for (key, value) in copies {
        if copy.insert(key.clone(), value.clone()).is_some() {
            bail!("--copy gives {key} twice");
        }
    }
    let asked = runtime::wait(client.ask_secrets(name, credential, copy, context))?;
    request::asked(&asked, json)
}

/// `KEY=VALUE`, a value a credential has the person enter at the provider,
/// split at its first `=`.
pub fn copy(arg: &str) -> Result<(String, String), String> {
    arg.split_once('=')
        .map(|(key, value)| (key.to_owned(), value.to_owned()))
        .ok_or_else(|| format!("{arg:?} is not KEY=VALUE"))
}

/// Ask the person at this terminal for `name`'s credentials, and set each
/// they give on the host the context names. `copies` are the values its
/// credentials have them enter at the provider.
pub fn fill(client: &Client, name: &str, copies: &[(String, String)]) -> Result<()> {
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        bail!(
            "crab secret fill takes values typed at a terminal: ask the person to run `crab secret fill {name}` in theirs"
        );
    }
    let credentials = runtime::wait(client.runtime(name))?.deps.grouped();
    if credentials.is_empty() {
        eprintln!("{name} declares no secrets");
        return Ok(());
    }
    let copy = copies.iter().cloned().collect();
    Form::new(client, name, credentials, &copy)?.run()
}
