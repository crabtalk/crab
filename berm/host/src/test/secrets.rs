use crate::{env::Secrets, test::Granted};
use anyhow::Result;

impl Secrets for Granted {
    async fn grant(&self, service: &str, secret: &str, host: &str) -> Result<Option<String>> {
        Ok(self
            .values
            .lock()
            .get(&(service.to_owned(), secret.to_owned(), host.to_owned()))
            .cloned())
    }

    async fn resource_owner(&self, service: &str, id: &str, host: &str) -> Result<Option<String>> {
        Ok(self
            .owners
            .lock()
            .get(&(service.into(), id.into(), host.into()))
            .cloned())
    }

    async fn names(&self, service: &str) -> Result<Vec<String>> {
        Ok(self
            .values
            .lock()
            .keys()
            .filter(|(granted, _, _)| granted == service)
            .map(|(_, secret, _)| secret.clone())
            .collect())
    }
}
