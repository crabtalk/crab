use crate::{
    client::{Client, Error},
    services::workflow::{Inspection, Run},
};
use berm_api::{host::Caller, workflow::view::Start};
use reqwest::Method;

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
