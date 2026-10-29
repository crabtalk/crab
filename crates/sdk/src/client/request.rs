//! `/requests` — asking a person to pay, to set a secret, to approve a deploy
//! or to verify on GitHub, by a code.

use crate::{
    account::request::{
        CreateRequest, DeviceAuthorization, DeviceToken, DeviceTokenRequest, Purchase, Request,
    },
    client::{Client, Error},
    services::service::DeployRequest,
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

    /// Ask a person to approve deploying the account's `services` together,
    /// by name.
    pub async fn ask_deploy(
        &self,
        services: BTreeMap<String, DeployRequest>,
        context: &str,
    ) -> Result<Request, Error> {
        self.ask(&CreateRequest::Deploy {
            account: self.account()?.name().to_owned(),
            services,
            context: context.to_owned(),
        })
        .await
    }

    /// Ask the person the client acts for to verify on GitHub.
    pub async fn ask_verify(&self, context: &str) -> Result<Request, Error> {
        self.ask(&CreateRequest::Verify {
            account: self.account()?.name().to_owned(),
            context: context.to_owned(),
        })
        .await
    }

    /// Ask an owner of the key's account to let this client's key call
    /// `services`, each as `owner/name`, or spend `budget` USD micros a UTC
    /// day.
    pub async fn ask_grant(
        &self,
        services: Vec<String>,
        budget: Option<i64>,
        context: &str,
    ) -> Result<Request, Error> {
        self.ask(&CreateRequest::Grant {
            services,
            budget,
            context: context.to_owned(),
        })
        .await
    }

    async fn ask(&self, body: &CreateRequest) -> Result<Request, Error> {
        self.json(self.at(Method::POST, "/requests").json(body))
            .await
    }

    /// End request `code` now, as if its time had run out. For the account it
    /// is for or an owner of it.
    pub async fn expire_request(&self, code: &str) -> Result<Request, Error> {
        self.json(self.at(Method::POST, &format!("/requests/{code}/expire")))
            .await
    }

    /// Ask a person to sign this device in. Takes no token.
    pub async fn ask_login(&self) -> Result<DeviceAuthorization, Error> {
        self.json(self.at(Method::POST, "/requests/login")).await
    }

    /// The session the sign-in `device_code` opened, once a person approved
    /// it; refused with a [`device`](crate::account::request::device) code
    /// until then. Takes no token.
    pub async fn login_token(&self, device_code: &str) -> Result<DeviceToken, Error> {
        let body = DeviceTokenRequest {
            device_code: device_code.to_owned(),
        };
        self.json(self.at(Method::POST, "/requests/login/token").json(&body))
            .await
    }

    /// The request `code` names, once its status changes while pending or
    /// `wait` seconds pass, at most 60; at once with `wait` 0. Takes no token.
    pub async fn request(&self, code: &str, wait: u64) -> Result<Request, Error> {
        let req = self
            .at(Method::GET, &format!("/requests/{code}"))
            .query(&[("wait", wait)]);
        self.json(req).await
    }
}
