use crate::{
    client::{Client, Error},
    services::workflow::{Inspection, Run},
};
use berm_api::{
    host::Caller,
    workflow::view::{Quote, Start},
};
use reqwest::Method;
use serde_json::Value;

impl Client {
    pub async fn workflow(
        &self,
        service: &str,
        workflow: &str,
        caller: Option<&Caller>,
    ) -> Result<Inspection, Error> {
        let path = format!("{}/workflows/{workflow}", self.path_of(service));
        self.json(self.as_caller(self.at(Method::GET, &path), caller)?)
            .await
    }

    /// What `input` settles for a run of `workflow` before it starts.
    pub async fn quote_workflow(
        &self,
        service: &str,
        workflow: &str,
        input: &Value,
        caller: Option<&Caller>,
    ) -> Result<Quote, Error> {
        let path = format!("{}/workflows/{workflow}/quote", self.path_of(service));
        self.json(self.as_caller(self.at(Method::POST, &path).json(input), caller)?)
            .await
    }

    pub async fn start_workflow(
        &self,
        service: &str,
        workflow: &str,
        request: &Start,
        caller: Option<&Caller>,
    ) -> Result<Run, Error> {
        let path = format!("{}/workflows/{workflow}/runs", self.path_of(service));
        self.json(self.as_caller(self.at(Method::POST, &path).json(request), caller)?)
            .await
    }

    pub async fn workflow_run(
        &self,
        service: &str,
        workflow: &str,
        run: &str,
    ) -> Result<Run, Error> {
        let path = format!("{}/workflows/{workflow}/runs/{run}", self.path_of(service));
        self.json(self.at(Method::GET, &path)).await
    }

    pub async fn continue_workflow(
        &self,
        service: &str,
        workflow: &str,
        run: &str,
    ) -> Result<Run, Error> {
        let path = format!(
            "{}/workflows/{workflow}/runs/{run}/continue",
            self.path_of(service)
        );
        self.json(self.at(Method::POST, &path)).await
    }

    pub async fn cancel_workflow(
        &self,
        service: &str,
        workflow: &str,
        run: &str,
    ) -> Result<Run, Error> {
        let path = format!(
            "{}/workflows/{workflow}/runs/{run}/cancel",
            self.path_of(service)
        );
        self.json(self.at(Method::POST, &path)).await
    }
}
