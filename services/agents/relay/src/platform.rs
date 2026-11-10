use alloc::string::String;
use berm_lang::{Caller, Result, Schema, abi::secrets::Id, secret};
use url::Url;

#[derive(Schema)]
pub enum Platform {
    Grok(GrokConnection),
    OpenAiDot(DotConnection),
}

#[derive(Schema)]
pub struct GrokConnection {
    pub webhook_url: String,
    /// A host-managed credential reference; never the webhook key itself.
    pub secret_id: String,
}

#[derive(Schema)]
pub struct DotConnection {}

impl Platform {
    pub fn validate(&self) -> Result<()> {
        if let Self::Grok(connection) = self {
            connection.validate()?;
        }
        Ok(())
    }
}

impl GrokConnection {
    pub fn validate(&self) -> Result<()> {
        let url = Url::parse(&self.webhook_url).map_err(|_| "Use an absolute HTTPS webhook URL")?;
        if self.webhook_url.len() > 4096
            || url.scheme() != "https"
            || url.host_str().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.fragment().is_some()
        {
            return Err("Use an HTTPS webhook URL up to 4096 bytes without embedded credentials or a fragment".into());
        }
        Id::try_from(self.secret_id.as_str())?;
        let Some(Caller::Outside(owner)) = berm_lang::caller()? else {
            return Err(berm_lang::deny(
                "Use an account session to bind a Grok connection",
            ));
        };
        if berm_lang::user()?.is_some()
            || secret::owner(
                &self.secret_id,
                url.host_str().ok_or("Missing webhook host")?,
            )?
            .as_deref()
                != Some(owner.as_str())
        {
            return Err(berm_lang::deny(
                "This account has no approved secret resource for the webhook host",
            ));
        }
        Ok(())
    }
}
