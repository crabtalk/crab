//! `…/keys` — the API keys an account holds.

use crate::{
    account::key::{CreateKeyRequest, CreateKeyResponse, Key, UpdateKeyRequest},
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

    /// Change a key's budget or scope.
    pub async fn update_key(&self, key_id: &str, body: &UpdateKeyRequest) -> Result<Key, Error> {
        let path = format!("{}/keys/{key_id}", self.account()?.api());
        self.json(self.at(Method::PATCH, &path).json(body)).await
    }

    /// Replace a key's token, keeping everything else of it. The new token
    /// comes back once; the old one stops working at once.
    pub async fn roll_key(&self, key_id: &str) -> Result<CreateKeyResponse, Error> {
        let path = format!("{}/keys/{key_id}/roll", self.account()?.api());
        self.json(self.at(Method::POST, &path)).await
    }

    /// Let a key call `service`, as `owner/name`.
    pub async fn grant_key(&self, key_id: &str, service: &str) -> Result<(), Error> {
        let path = format!("{}/keys/{key_id}/services/{service}", self.account()?.api());
        self.ok(self.at(Method::PUT, &path)).await?;
        Ok(())
    }

    /// Stop a key calling `service`, as `owner/name`.
    pub async fn revoke_grant(&self, key_id: &str, service: &str) -> Result<(), Error> {
        let path = format!("{}/keys/{key_id}/services/{service}", self.account()?.api());
        self.ok(self.at(Method::DELETE, &path)).await?;
        Ok(())
    }

    /// Revoke a key by id.
    pub async fn delete_key(&self, key_id: &str) -> Result<(), Error> {
        let path = format!("{}/keys/{key_id}", self.account()?.api());
        self.ok(self.at(Method::DELETE, &path)).await?;
        Ok(())
    }
}
