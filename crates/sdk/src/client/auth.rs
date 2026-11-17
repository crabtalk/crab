use crate::{
    account::auth::Status,
    client::{Client, Error},
};
use reqwest::Method;

impl Client {
    pub async fn auth_status(&self) -> Result<Status, Error> {
        self.json(self.at(Method::GET, "/auth/status")).await
    }
}
