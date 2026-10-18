use crate::host::Unrecorded;
use anyhow::Result;
use berm::{Records, Storage};

impl Storage for Unrecorded {
    fn put(&self, _: Records, _: &str, _: &[u8]) -> Result<()> {
        Ok(())
    }

    fn remove(&self, _: Records, _: &str) -> Result<bool> {
        Ok(false)
    }

    fn list(&self, _: Records) -> Result<Vec<(String, Vec<u8>)>> {
        Ok(Vec::new())
    }
}
