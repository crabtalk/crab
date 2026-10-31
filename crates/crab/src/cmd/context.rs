//! `crab context` — name the hosts `crab` talks to, and pick one.

use crate::{
    context::{self, Context, Contexts},
    terminal,
};
use anyhow::{Context as _, Result, bail};
use crabtalk_sdk::client::Owner;
use serde::Serialize;
use std::io::Read;

/// Name `host` as `name`: a local host, or with a key and `org` or `user`, Cloud.
/// The key is `token`, or read from stdin when `token_stdin`.
pub fn add(
    name: &str,
    host: &str,
    token: Option<&str>,
    token_stdin: bool,
    org: Option<&str>,
    user: Option<&str>,
) -> Result<()> {
    let owner = match (org, user) {
        (Some(org), _) => Some(Owner::Org(org.to_owned())),
        (_, Some(user)) => Some(Owner::User(user.to_owned())),
        _ => None,
    };
    let token = match token_stdin {
        true => {
            let mut token = String::new();
            std::io::stdin()
                .read_to_string(&mut token)
                .context("reading the key from stdin")?;
            Some(token.trim().to_owned())
        }
        false => token.map(str::to_owned),
    };
    if name == context::DEFAULT {
        bail!(
            "{:?} is this machine's local host and cannot be replaced",
            context::DEFAULT
        );
    }
    if owner.is_some() != token.is_some() {
        bail!("a Cloud context takes both `--token` and `--org` or `--user`");
    }
    let mut contexts = Contexts::load()?;
    contexts.contexts.insert(
        name.to_owned(),
        Context {
            host: host.to_owned(),
            token,
            owner,
        },
    );
    contexts.save()?;
    eprintln!("added {name}");
    Ok(())
}

/// Send every later call to `name`.
pub fn use_(name: &str) -> Result<()> {
    let mut contexts = Contexts::load()?;
    contexts.get(Some(name))?;
    contexts.current = Some(name.to_owned());
    contexts.save()?;
    eprintln!("using {name}");
    Ok(())
}

/// One context as `crab context ls --json` lists it, its token left out.
#[derive(Serialize)]
struct Listed<'a> {
    name: &'a str,
    host: &'a str,
    owner: &'a Option<Owner>,
    signed_in: bool,
    current: bool,
}

pub fn ls(json: bool) -> Result<()> {
    let contexts = Contexts::load()?;
    let current = contexts.current.as_deref().unwrap_or(context::DEFAULT);
    let default = contexts.get(Some(context::DEFAULT))?;
    let rows = std::iter::once((context::DEFAULT, &default)).chain(
        contexts
            .contexts
            .iter()
            .map(|(name, at)| (name.as_str(), at)),
    );
    if json {
        let listed: Vec<Listed> = rows
            .map(|(name, at)| Listed {
                name,
                host: &at.host,
                owner: &at.owner,
                signed_in: at.token.is_some(),
                current: name == current,
            })
            .collect();
        return terminal::json(&listed);
    }
    for (name, at) in rows {
        let mark = if name == current { "*" } else { " " };
        match &at.owner {
            Some(Owner::Org(org)) => println!("{mark} {name}  {}  org {org}", at.host),
            Some(Owner::User(user)) => println!("{mark} {name}  {}  user {user}", at.host),
            None => println!("{mark} {name}  {}", at.host),
        }
    }
    Ok(())
}

pub fn rm(name: &str) -> Result<()> {
    let mut contexts = Contexts::load()?;
    if contexts.contexts.remove(name).is_none() {
        bail!("no context named {name:?}");
    }
    if contexts.current.as_deref() == Some(name) {
        contexts.current = None;
    }
    contexts.save()?;
    eprintln!("removed {name}");
    Ok(())
}
