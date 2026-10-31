//! `/meta` — what the deployment is, and what it serves.

use crate::{
    account::billing::PublicPlan,
    client::{Client, Error},
    system::meta::{Explanation, Metering, VersionInfo},
};
use reqwest::Method;

impl Client {
    /// The version gate: the oldest client this deployment serves, and what it
    /// is running.
    pub async fn version(&self) -> Result<VersionInfo, Error> {
        self.json(self.at(Method::GET, "/meta/version")).await
    }

    /// Every plan it sells.
    pub async fn plans(&self) -> Result<Vec<PublicPlan>, Error> {
        self.json(self.at(Method::GET, "/meta/plans")).await
    }

    /// What error code `code` means, as `E0001`.
    pub async fn explain(&self, code: &str) -> Result<Explanation, Error> {
        self.json(self.at(Method::GET, &format!("/meta/errors/{code}")))
            .await
    }

    /// What metering costs a service's owner.
    pub async fn rates(&self) -> Result<Metering, Error> {
        self.json(self.at(Method::GET, "/meta/rates")).await
    }

    /// Whether the deployment is serving, by status code alone: a `503` is
    /// `false`.
    pub async fn health(&self) -> Result<bool, Error> {
        self.probe(self.at(Method::GET, "/meta/health")).await
    }
}
