//! `/meta` — what the deployment is, and what it serves.

use crate::{
    account::billing::PublicPlan,
    client::{Client, Error},
    system::meta::{Metering, VersionInfo},
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

    /// What metering costs a service's owner.
    pub async fn rates(&self) -> Result<Metering, Error> {
        self.json(self.at(Method::GET, "/meta/rates")).await
    }

    /// Whether the deployment is serving. Answered by status code alone, so a
    /// `503` is a `false` rather than an [`Error`].
    pub async fn health(&self) -> Result<bool, Error> {
        self.probe(self.at(Method::GET, "/meta/health")).await
    }
}
