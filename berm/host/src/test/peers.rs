use crate::{env::Peers, test::Nowhere};
use anyhow::{Result, bail};
use berm_api::host::{Link, Reply, Used};

impl Peers for Nowhere {
    async fn link(&self, peer: &str, _: &str, _: &str, _: Vec<u8>, _: Link) -> Result<Reply> {
        bail!("no peer named {peer:?}")
    }

    async fn ship(&self, _: &str, _: &[Used]) -> Result<()> {
        Ok(())
    }
}
