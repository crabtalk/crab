use alloc::string::String;
use berm_lang::{Result, Schema, abi::secrets::Id};
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
        Ok(())
    }
}
