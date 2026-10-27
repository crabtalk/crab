//! An account's plan, period, credit and payments. Each call that takes a card
//! answers a Stripe URL for the caller to open.

use crate::{
    account::{
        auth::Client as Surface,
        billing::{
            CheckoutResponse, Credit, Invoice, PaymentMethod, Period, PortalFlow, PortalResponse,
            RefillRequest, SubscribeResponse, TrialOffer,
        },
    },
    client::{Client, Error},
    sandbox::service::CostedDay,
};
use berm_api::host::Memory;
use reqwest::Method;
use std::collections::BTreeMap;

impl Client {
    /// What the account used of its plan this period.
    pub async fn period(&self) -> Result<Period, Error> {
        let path = format!("{}/billing/usage", self.account()?.path());
        self.json(self.at(Method::GET, &path)).await
    }

    /// Each of the account's services over the last 30 days, by name, a day
    /// each, oldest first.
    pub async fn services_usage(&self) -> Result<BTreeMap<String, Vec<CostedDay>>, Error> {
        let path = format!("{}/services/usage", self.account()?.path());
        self.json(self.at(Method::GET, &path)).await
    }

    /// The memory the account's services hold now, and its plan's limit.
    pub async fn memory(&self) -> Result<Memory, Error> {
        let path = format!("{}/memory", self.account()?.path());
        self.json(self.at(Method::GET, &path)).await
    }

    pub async fn credit(&self) -> Result<Credit, Error> {
        let path = format!("{}/billing/credit", self.account()?.api());
        self.json(self.at(Method::GET, &path)).await
    }

    /// A Checkout URL buying credit, its amount chosen there.
    pub async fn buy_credit(&self, surface: Surface) -> Result<CheckoutResponse, Error> {
        let path = format!("{}/billing/credit", self.account()?.api());
        let req = self
            .at(Method::POST, &path)
            .query(&[("client", surface.as_ref())]);
        self.json(req).await
    }

    /// Subscribe to `plan`: a Checkout URL, or none when a trial ended into it.
    pub async fn checkout(&self, plan: &str, surface: Surface) -> Result<SubscribeResponse, Error> {
        let path = format!("{}/billing/checkout", self.account()?.api());
        let req = self
            .at(Method::POST, &path)
            .query(&[("client", surface.as_ref()), ("plan", plan)]);
        self.json(req).await
    }

    /// The caller's free trial as it would start now. A person's only.
    pub async fn trial_offer(&self) -> Result<TrialOffer, Error> {
        self.json(self.at(Method::GET, "/user/billing/trial")).await
    }

    /// A Checkout URL starting the caller's free trial. A person's only.
    pub async fn trial(&self, surface: Surface) -> Result<CheckoutResponse, Error> {
        let req = self
            .at(Method::POST, "/user/billing/trial")
            .query(&[("client", surface.as_ref())]);
        self.json(req).await
    }

    pub async fn set_refill(&self, body: &RefillRequest) -> Result<(), Error> {
        let path = format!("{}/billing/refill", self.account()?.api());
        self.ok(self.at(Method::PUT, &path).json(body)).await?;
        Ok(())
    }

    /// The account's invoices, newest first.
    pub async fn invoices(&self) -> Result<Vec<Invoice>, Error> {
        let path = format!("{}/billing/invoices", self.account()?.api());
        self.json(self.at(Method::GET, &path)).await
    }

    /// The card the account's subscription charges, if any.
    pub async fn payment_method(&self) -> Result<Option<PaymentMethod>, Error> {
        let path = format!("{}/billing/payment-method", self.account()?.api());
        self.json(self.at(Method::GET, &path)).await
    }

    /// A Stripe Customer Portal URL, opened on `flow` when one is given.
    pub async fn portal(
        &self,
        surface: Surface,
        flow: Option<PortalFlow>,
    ) -> Result<PortalResponse, Error> {
        let path = format!("{}/billing/portal", self.account()?.api());
        let mut req = self
            .at(Method::POST, &path)
            .query(&[("client", surface.as_ref())]);
        if let Some(flow) = flow {
            req = req.query(&[("flow", flow.as_str())]);
        }
        self.json(req).await
    }
}
