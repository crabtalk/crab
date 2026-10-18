use crate::{env::Images, test::Given};
use anyhow::{Context, Result};

impl Images for Given {
    async fn pull(&self, reference: &str, _: &str) -> Result<Vec<u8>> {
        self.0
            .lock()
            .get(reference)
            .cloned()
            .with_context(|| format!("no image given as {reference}"))
    }
}
