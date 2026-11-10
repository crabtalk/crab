use crate::services::service::SetSecret;
use berm_abi::{self as abi, secrets};
use berm_api::{Credential, Deps, digest};
use serde::{Deserialize, Serialize};
use std::fmt::{Debug, Formatter, Result as FmtResult};

pub const PATH: &str = "/secrets";

pub const GRANT_REFUSED: &str = "E0050";

#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[cfg_attr(feature = "utoipa", schema(as = SecretResource))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Resource {
    pub id: String,
    pub name: String,
    pub created_at: i64,
    pub grants: Vec<Grant>,
}

impl Resource {
    pub fn check_name(name: &str) -> Result<(), String> {
        if name.trim().is_empty() || name.len() > 128 || name.chars().any(char::is_control) {
            return Err("Use a secret name of 1–128 bytes on one line".into());
        }
        Ok(())
    }

    pub fn new(name: String, random: [u8; 16], created_at: i64) -> Self {
        Self {
            id: format!("{}{}", secrets::PREFIX, digest::hex(&random)),
            name,
            created_at,
            grants: Vec::new(),
        }
    }
}

#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[cfg_attr(feature = "utoipa", schema(as = CreateSecretResource))]
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Create {
    pub name: String,
    pub value: String,
}
impl Debug for Create {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.debug_struct("Create")
            .field("name", &self.name)
            .field("value", &"[redacted]")
            .finish()
    }
}
impl Create {
    pub fn check(&self) -> Result<(), String> {
        Resource::check_name(&self.name)?;
        SetSecret::check_value(&self.value)?;
        Ok(())
    }
}

#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[cfg_attr(feature = "utoipa", schema(as = SecretResourceGrant))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Grant {
    pub service: String,
    pub hosts: Vec<String>,
}
impl Grant {
    pub fn check(&self, deps: &Deps) -> Result<(), String> {
        Credential::check_hosts(&self.hosts)?;
        if self.hosts.is_empty() {
            return Err("Approve at least one destination host".into());
        }
        if !deps.secret_resources {
            return Err("The service does not declare secret_resources = true".into());
        }
        if self.hosts.iter().any(|host| {
            deps.http.reach(abi::HTTP_FETCH, host).is_none()
                && deps.http.reach(abi::HTTP_PROXY, host).is_none()
        }) {
            return Err(
                "The secret destinations must be allowed by the service's HTTP permissions".into(),
            );
        }
        Ok(())
    }
}

#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[cfg_attr(feature = "utoipa", schema(as = SecretResourceSetup))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Setup {
    pub name: String,
    pub hosts: Vec<String>,
    #[serde(default)]
    pub id: Option<String>,
}
