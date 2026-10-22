//! `/requests` — asking a person to pay or to set a secret, by a code.

use crate::{
    account::request::{CreateRequest, Purchase, Request},
    client::{Client, Error},
};
use reqwest::Method;
use std::collections::BTreeMap;

impl Client {
    /// Ask a person to pay for `purchase`, of `plan` when it is one, for the
    /// account the client acts for.
    pub async fn ask_payment(
        &self,
        purchase: Purchase,
        plan: Option<&str>,
        context: &str,
    ) -> Result<Request, Error> {
        self.ask(&CreateRequest::Payment {
            account: self.account()?.name().to_owned(),
            purchase,
            plan: plan.map(str::to_owned),
            context: context.to_owned(),
        })
        .await
    }

    /// Ask a person to set the unset secrets of the account's service
    /// `service`, or its credential `credential`, with `copy` the values its
    /// credentials have them enter at the provider.
    pub async fn ask_secrets(
        &self,
        service: &str,
        credential: Option<&str>,
        copy: BTreeMap<String, String>,
        context: &str,
    ) -> Result<Request, Error> {
        self.ask(&CreateRequest::Secret {
            account: self.account()?.name().to_owned(),
            service: service.to_owned(),
            credential: credential.map(str::to_owned),
            copy,
            context: context.to_owned(),
        })
        .await
    }

    async fn ask(&self, body: &CreateRequest) -> Result<Request, Error> {
        self.json(self.at(Method::POST, "/requests").json(body))
            .await
    }

    /// The request `code` names. Takes no token.
    pub async fn request(&self, code: &str) -> Result<Request, Error> {
        self.json(self.at(Method::GET, &format!("/requests/{code}")))
            .await
    }
}
