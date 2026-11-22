use alloc::string::String;
use berm_lang::{Result, Schema, abi::secrets::Id, secret};
use url::Url;

pub(crate) const GROK_HOST: &str = "api2.cursor.sh";
const GROK_PATH: &str = "/automations/webhook/";

#[derive(Default, Schema, PartialEq, Eq)]
pub enum Platform {
    #[default]
    Inbox,
    Grok(GrokConnection),
    OpenAiDot(DotConnection),
}

#[derive(Schema, PartialEq, Eq)]
pub struct GrokConnection {
    pub webhook_url: String,
    /// A host-managed credential reference; never the webhook key itself.
    pub secret_id: String,
}

#[derive(Schema, PartialEq, Eq)]
pub struct DotConnection {}

impl Platform {
    pub fn validate(&mut self) -> Result<()> {
        if let Self::Grok(connection) = self {
            connection.validate()?;
        }
        Ok(())
    }

    pub(crate) fn normalize_url(&mut self) -> Result<()> {
        if let Self::Grok(connection) = self {
            connection.normalize_url()?;
        }
        Ok(())
    }
}

impl GrokConnection {
    fn normalize_url(&mut self) -> Result<()> {
        let url = Url::parse(&self.webhook_url).map_err(|_| "Use an absolute HTTPS webhook URL")?;
        if self.webhook_url.len() > 4096
            || url.scheme() != "https"
            || url.host_str() != Some(GROK_HOST)
            || url.port().is_some()
            || !url.path().starts_with(GROK_PATH)
            || !url.username().is_empty()
            || url.password().is_some()
            || url.fragment().is_some()
        {
            return Err("Use an HTTPS api2.cursor.sh/automations/webhook/ URL up to 4096 bytes without embedded credentials or a fragment".into());
        }
        self.webhook_url = String::from(url.as_str());
        Ok(())
    }

    pub fn validate(&mut self) -> Result<()> {
        self.normalize_url()?;
        Id::try_from(self.secret_id.as_str())?;
        if secret::owner(&self.secret_id, GROK_HOST)?.is_none() {
            return Err(berm_lang::deny(
                "This account has no approved secret resource for the webhook host",
            ));
        }
        Ok(())
    }
}
