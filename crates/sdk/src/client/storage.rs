use crate::client::{Client, Error};
use berm_api::storage::{Caller, Usage};
use reqwest::{Method, Response};

impl Client {
    pub async fn stored_callers(&self, service: &str) -> Result<Vec<Caller>, Error> {
        self.json(self.at(Method::GET, &format!("{}/callers", self.path_of(service))))
            .await
    }

    pub async fn caller_storage(&self) -> Result<Usage, Error> {
        let path = format!("{}/storage", self.account()?.api());
        self.json(self.at(Method::GET, &path)).await
    }

    pub async fn download_caller_storage(&self, id: &str) -> Result<Response, Error> {
        let path = format!("{}/storage/{id}", self.account()?.api());
        self.stream(self.at(Method::GET, &path)).await
    }

    pub async fn delete_caller_storage(&self, id: &str) -> Result<(), Error> {
        let path = format!("{}/storage/{id}", self.account()?.api());
        self.ok(self.at(Method::DELETE, &path)).await.map(drop)
    }
}
