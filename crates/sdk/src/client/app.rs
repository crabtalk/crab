//! `…/apps` — the apps a website calls an account's services through.

use crate::{
    account::app::{App, AppRequest},
    client::{Client, Error},
};
use reqwest::Method;

impl Client {
    pub async fn apps(&self) -> Result<Vec<App>, Error> {
        let path = format!("{}/apps", self.account()?.api());
        self.json(self.at(Method::GET, &path)).await
    }

    pub async fn create_app(&self, body: &AppRequest) -> Result<App, Error> {
        let path = format!("{}/apps", self.account()?.api());
        self.json(self.at(Method::POST, &path).json(body)).await
    }

    /// Replace app `id` with `body`.
    pub async fn update_app(&self, id: &str, body: &AppRequest) -> Result<App, Error> {
        let path = format!("{}/apps/{id}", self.account()?.api());
        self.json(self.at(Method::PUT, &path).json(body)).await
    }

    pub async fn delete_app(&self, id: &str) -> Result<(), Error> {
        let path = format!("{}/apps/{id}", self.account()?.api());
        self.ok(self.at(Method::DELETE, &path)).await?;
        Ok(())
    }
}
