//! `crab secret` — a program's secrets on Cloud.

use crate::{cmd::request, utils};
use anyhow::{Context, Result, bail};
use crabtalk_sdk::{account::request::AskedSecret, client::Client};
use std::io::Read;

/// Set `name`'s secret `secret` to the value on stdin, its trailing newline
/// dropped.
pub fn put(client: &Client, name: &str, secret: &str) -> Result<()> {
    let mut value = String::new();
    std::io::stdin()
        .read_to_string(&mut value)
        .context("reading the secret's value from stdin")?;
    let value = value.trim_end_matches(['\n', '\r']);
    if value.is_empty() {
        bail!("no value on stdin: `printf %s \"$VALUE\" | crab secret put {name} {secret}`");
    }
    utils::wait(client.put_secret(name, secret, value))?;
    eprintln!("set {name} {secret}");
    Ok(())
}

/// Print `name`'s secrets, by name.
pub fn ls(client: &Client, name: &str, json: bool) -> Result<()> {
    let secrets = utils::wait(client.secrets(name))?;
    if json {
        return utils::json(&secrets);
    }
    for secret in &secrets {
        println!("{}", secret.name);
    }
    Ok(())
}

/// Revoke `name`'s secret `secret`.
pub fn rm(client: &Client, name: &str, secret: &str, yes: bool) -> Result<()> {
    utils::confirm(yes, &format!("revoke {name} {secret}"))?;
    utils::wait(client.delete_secret(name, secret))?;
    eprintln!("revoked {name} {secret}");
    Ok(())
}

/// Ask a person to set `name`'s `secrets`, each beside its guide in `guides`
/// when it has one, with `context` shown to them: print its page, and how to
/// wait for it.
pub fn ask(
    client: &Client,
    name: &str,
    secrets: &[String],
    guides: &[(String, String)],
    context: &str,
    json: bool,
) -> Result<()> {
    if let Some((stray, _)) = guides.iter().find(|(secret, _)| !secrets.contains(secret)) {
        bail!("--guide names {stray}, which is not asked for");
    }
    let asked = secrets
        .iter()
        .map(|secret| AskedSecret {
            name: secret.clone(),
            guide: guides
                .iter()
                .find(|(named, _)| named == secret)
                .map(|(_, guide)| guide.clone())
                .unwrap_or_default(),
        })
        .collect();
    let asked = utils::wait(client.ask_secrets(name, asked, context))?;
    request::asked(&asked, json)
}

/// `SECRET=TEXT`, a guide to getting a secret, split at its first `=`.
pub fn guide(arg: &str) -> Result<(String, String), String> {
    arg.split_once('=')
        .map(|(secret, text)| (secret.to_owned(), text.to_owned()))
        .ok_or_else(|| format!("{arg:?} is not SECRET=TEXT"))
}
