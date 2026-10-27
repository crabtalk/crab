use crate::{env::Images, test::Given};
use anyhow::{Context, Result};

impl Images for Given {
    async fn pull(&self, _: &str, digest: &str) -> Result<Vec<u8>> {
        self.0
            .lock()
            .get(digest)
            .cloned()
            .with_context(|| format!("no image given as {digest}"))
    }
}
