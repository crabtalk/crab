use crate::database::Database;
use anyhow::Result;
use rusqlite::MAIN_DB;
use std::path::PathBuf;

impl Database {
    /// Copy a committed SQLite snapshot, including key-value state.
    pub async fn export(&self, path: PathBuf) -> Result<()> {
        self.readers
            .with(move |conn| {
                conn.backup(MAIN_DB, path, None)?;
                Ok(())
            })
            .await
    }
}
