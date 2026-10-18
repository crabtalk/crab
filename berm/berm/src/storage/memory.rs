use crate::storage::{Memory, Records, Storage};
use anyhow::Result;

impl Storage for Memory {
    fn put(&self, records: Records, key: &str, value: &[u8]) -> Result<()> {
        self.held
            .lock()
            .insert((records, key.to_owned()), value.to_vec());
        Ok(())
    }

    fn remove(&self, records: Records, key: &str) -> Result<bool> {
        Ok(self
            .held
            .lock()
            .remove(&(records, key.to_owned()))
            .is_some())
    }

    fn list(&self, records: Records) -> Result<Vec<(String, Vec<u8>)>> {
        Ok(self
            .held
            .lock()
            .iter()
            .filter(|((kind, _), _)| *kind == records)
            .map(|((_, key), value)| (key.clone(), value.clone()))
            .collect())
    }
}
