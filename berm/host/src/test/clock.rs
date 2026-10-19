use crate::{env::Clock, test::Manual};
use std::sync::atomic::Ordering;

impl Clock for Manual {
    fn now(&self) -> u64 {
        self.0.load(Ordering::Relaxed)
    }
}
