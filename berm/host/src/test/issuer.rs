use crate::{env::Issuer, test::Generated};
use anyhow::Result;

impl Issuer for Generated {
    async fn issuer(&self, service: &str) -> Result<String> {
        Ok(format!("http://issuer.test/{service}"))
    }

    async fn kid(&self, service: &str) -> Result<String> {
        Ok(self.key(service)?.kid().to_owned())
    }

    async fn sign(&self, service: &str, message: &[u8]) -> Result<Vec<u8>> {
        Ok(self.key(service)?.sign(message))
    }
}
