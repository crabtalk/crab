//! `/auth/email` — signing in with a code emailed to the address.

use crate::{
    account::auth::{Client as Surface, CodeRequest, VerifyRequest, VerifyResponse},
    client::{Client, Error},
};
use reqwest::Method;

impl Client {
    /// Email a sign-in code to `email`. Cloud answers the same whether or not
    /// the address has an account.
    pub async fn send_code(&self, email: &str) -> Result<(), Error> {
        let body = CodeRequest {
            email: email.to_owned(),
        };
        self.ok(self.at(Method::POST, "/auth/email").json(&body))
            .await?;
        Ok(())
    }

    /// Spend the code emailed to `email` for a session opened for `surface`.
    /// Creates the account when the address has none.
    pub async fn verify_code(
        &self,
        email: &str,
        code: &str,
        surface: Surface,
    ) -> Result<VerifyResponse, Error> {
        let body = VerifyRequest {
            email: email.to_owned(),
            code: code.to_owned(),
            client: surface,
        };
        self.json(self.at(Method::POST, "/auth/email/verify").json(&body))
            .await
    }
}
