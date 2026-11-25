//! Sign in to Cloud by browser approval and keep the credential as a context.

use crate::{
    context::{self, Context, Contexts},
    runtime, terminal,
};
use anyhow::{Result, bail};
use crabtalk_sdk::{
    account::{org::PublicAccount, request::device},
    client::{Client, Error, Owner},
};
use std::{thread, time::Duration};

/// The Cloud `crab login` signs in to unless told another.
pub const CLOUD: &str = "https://api.crabtalk.ai";

pub fn run(name: &str, host: &str, device: bool) -> Result<()> {
    if name == context::DEFAULT {
        bail!("{:?} is this machine's local host", context::DEFAULT);
    }
    let host = host.trim_end_matches('/');
    let client = Client::new(host);
    let asked = if device {
        runtime::wait(client.ask_agent_login(&format!("crab ({name})")))?
    } else {
        runtime::wait(client.ask_login())?
    };
    eprintln!(
        "sign crab in: open the link, or enter {} at {}, within {} minutes",
        asked.user_code,
        asked.verification_uri,
        asked.expires_in / 60
    );
    if device || !terminal::open(&asked.verification_uri_complete) {
        println!("{}", asked.verification_uri_complete);
    }
    let mut interval = Duration::from_secs(asked.interval);
    let signed = loop {
        thread::sleep(interval);
        match runtime::wait(client.login_token(&asked.device_code)) {
            Ok(signed) => break signed,
            Err(Error::Api { status: 429, .. }) => interval += Duration::from_secs(5),
            Err(Error::Api {
                code: Some(code), ..
            }) => match code.as_str() {
                device::AUTHORIZATION_PENDING => {}
                device::SLOW_DOWN => interval += Duration::from_secs(5),
                device::ACCESS_DENIED => bail!("the person declined the sign-in"),
                device::EXPIRED_TOKEN => bail!("the code expired; run `crab login` again"),
                other => bail!("sign-in failed: {other}"),
            },
            Err(e) => return Err(e.into()),
        }
    };
    keep(name, host, signed.token, signed.account)
}

/// Keep `token` as context `name` on `host`, and use it from now on.
fn keep(name: &str, host: &str, token: String, account: Option<PublicAccount>) -> Result<()> {
    let (id, handle) = match account {
        Some(account) => (account.id, account.handle),
        None => {
            let me = runtime::wait(Client::new(host).token(&token).me())?;
            (me.user.id, me.user.handle)
        }
    };
    let who = handle.unwrap_or_else(|| id.clone());
    let mut contexts = Contexts::load()?;
    contexts.contexts.insert(
        name.to_owned(),
        Context {
            host: host.to_owned(),
            token: Some(token),
            account_id: Some(id),
            owner: Some(Owner::User(who.clone())),
        },
    );
    contexts.current = Some(name.to_owned());
    contexts.save()?;
    eprintln!("signed in as {who} on {host}, as context {name}");
    Ok(())
}
