use crate::{env::Secrets, test::Granted};
use anyhow::Result;

impl Secrets for Granted {
    async fn grant(&self, service: &str, secret: &str, host: &str) -> Result<Option<String>> {
        Ok(self
            .0
            .lock()
            .get(&(service.to_owned(), secret.to_owned(), host.to_owned()))
            .cloned())
    }

    async fn names(&self, service: &str) -> Result<Vec<String>> {
        Ok(self
            .0
            .lock()
            .keys()
            .filter(|(granted, _, _)| granted == service)
            .map(|(_, secret, _)| secret.clone())
            .collect())
    }
}
