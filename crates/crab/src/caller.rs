use anyhow::{Result, ensure};
use berm_api::host::{Caller, User};
use clap::Args;
use crabtalk_sdk::client::{self, Client};

#[derive(Args)]
pub struct Identity {
    /// On the local host, override the default caller `local`.
    #[arg(long)]
    pub caller: Option<String>,
    /// On the local host, call without a caller.
    #[arg(long, conflicts_with_all = ["caller", "issuer", "user"])]
    pub anonymous: bool,
    /// On the local host, the end user's issuer (unchecked).
    #[arg(long, requires = "user")]
    pub issuer: Option<String>,
    /// On the local host, the end user's subject (unchecked).
    #[arg(long, requires = "issuer")]
    pub user: Option<String>,
}

impl Identity {
    pub fn resolve(&self, client: &Client) -> Result<Option<Caller>> {
        if client.owner.is_some() {
            ensure!(
                !self.anonymous
                    && self.caller.is_none()
                    && self.issuer.is_none()
                    && self.user.is_none(),
                "caller identity flags are only available on the local host; Cloud uses your credential"
            );
            return Ok(None);
        }
        Ok((!self.anonymous).then(|| Caller {
            id: self
                .caller
                .clone()
                .unwrap_or_else(|| client::LOCAL.to_owned()),
            app: None,
            user: self
                .issuer
                .clone()
                .zip(self.user.clone())
                .map(|(issuer, subject)| User { issuer, subject }),
            client: None,
            agent: None,
            key: None,
        }))
    }
}
