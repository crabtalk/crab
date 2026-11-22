use berm_lang::{Result, host};

#[derive(Clone, Copy)]
pub(crate) struct Window {
    pub since: i64,
    pub until: i64,
}

impl Window {
    pub fn new(since: u64, until: Option<u64>) -> Result<Self> {
        let since =
            i64::try_from(since).map_err(|_| "since exceeds the supported timestamp range")?;
        let until = i64::try_from(until.unwrap_or_else(|| host::now().div_ceil(1000)))
            .map_err(|_| "until exceeds the supported timestamp range")?;
        Ok(Self { since, until })
    }

    pub fn previous(self) -> Self {
        Self {
            since: self.since - (self.until - self.since).max(0),
            until: self.since,
        }
    }
}
