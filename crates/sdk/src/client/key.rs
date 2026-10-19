//! `…/keys` — the API keys an account holds.

use crate::{
    account::key::{CreateKeyRequest, CreateKeyResponse, Key},
    client::{Client, Error},
};
use reqwest::Method;

impl Client {
    /// The account's keys, active and revoked.
    pub async fn keys(&self) -> Result<Vec<Key>, Error> {
        let path = format!("{}/keys", self.account()?.api());
        self.json(self.at(Method::GET, &path)).await
    }

    /// Mint a key. The raw token comes back once and is never retrievable
    /// again.
    pub async fn create_key(&self, body: &CreateKeyRequest) -> Result<CreateKeyResponse, Error> {
        let path = format!("{}/keys", self.account()?.api());
        self.json(self.at(Method::POST, &path).json(body)).await
    }

    /// Revoke a key by id.
    pub async fn delete_key(&self, key_id: &str) -> Result<(), Error> {
        let path = format!("{}/keys/{key_id}", self.account()?.api());
        self.ok(self.at(Method::DELETE, &path)).await?;
        Ok(())
    }
}
