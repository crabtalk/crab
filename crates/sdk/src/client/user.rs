//! `/user` — the account the token names.

use crate::{
    account::user::AuthMe,
    client::{Client, Error},
};
use reqwest::Method;

impl Client {
    /// The account the token names, with what funds it and what it has spent.
    pub async fn me(&self) -> Result<AuthMe, Error> {
        self.json(self.at(Method::GET, "/user")).await
    }
}
