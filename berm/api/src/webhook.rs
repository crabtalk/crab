use crate::{Credential, CredentialSecret, Deps, ToolSpec};
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Scheme {
    Standard,
    Stripe,
}

impl std::fmt::Display for Scheme {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Standard => "standard",
            Self::Stripe => "stripe",
        })
    }
}

impl Scheme {
    pub fn credential(self, tool: &str) -> Credential {
        let endpoint = format!("https://<cloud>/services/<owner>/<service>/webhooks/{tool}");
        let (title, guide) = match self {
            Self::Standard => (
                "Webhook signing secret",
                format!(
                    "Use a Standard Webhooks signing secret shared with your sender for {endpoint}. Cloud verifies webhook-id, webhook-timestamp and webhook-signature before {tool} runs."
                ),
            ),
            Self::Stripe => (
                "Stripe webhook signing secret",
                format!(
                    "Add a webhook endpoint at {endpoint} subscribed to checkout.session.completed, customer.subscription.created, customer.subscription.updated and customer.subscription.deleted. Copy the endpoint's signing secret from the Stripe dashboard."
                ),
            ),
        };
        Credential {
            title: title.into(),
            guide,
            copy: BTreeMap::new(),
            hosts: Vec::new(),
            optional: false,
            secrets: vec![CredentialSecret {
                name: ToolSpec::signing_secret(tool),
                label: "Signing secret".into(),
                masked: true,
                pattern: None,
            }],
        }
    }
}

impl ToolSpec {
    pub fn signing_secret(tool: &str) -> String {
        format!("{}_WEBHOOK_SECRET", tool.to_ascii_uppercase())
    }
    pub fn signing_credential(tool: &str) -> String {
        format!("{tool}_webhook")
    }
    pub fn check_signing_name(&self, name: &str) -> Result<()> {
        ensure!(
            name != Self::signing_credential(&self.name)
                && name != Self::signing_secret(&self.name),
            "`{name}` is the signing secret of the webhook tool `{}`; remove it from crab.toml.",
            self.name
        );
        Ok(())
    }
}

impl Deps {
    pub fn add_signing(&mut self, tools: &[ToolSpec]) -> Result<()> {
        for tool in tools.iter().filter(|tool| tool.webhook.is_some()) {
            for name in self.credentials.keys().chain(self.secrets.iter()) {
                tool.check_signing_name(name)?;
            }
        }
        for tool in tools {
            if let Some(scheme) = tool.webhook {
                let secret = ToolSpec::signing_secret(&tool.name);
                ensure!(
                    !self.secrets.contains(&secret),
                    "webhook tools share signing secret {secret}"
                );
                self.secrets.push(secret);
                self.credentials.insert(
                    ToolSpec::signing_credential(&tool.name),
                    scheme.credential(&tool.name),
                );
            }
        }
        Ok(())
    }
}
