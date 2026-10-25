//! How a host is sized.

use berm::syscall::call::DEFAULT_CALL_DEPTH;
use serde::Deserialize;
use std::{path::PathBuf, time::Duration};

/// A missing field takes its default.
#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    /// Guests live at once across every service, nested calls included.
    #[serde(default = "instances")]
    pub instances: u32,
    /// The largest linear memory one guest may grow to, in bytes.
    #[serde(default = "memory")]
    pub memory: usize,
    /// The most CPU time one invocation may spend, nested calls excluded, as a
    /// duration like `"50ms"`.
    #[serde(default = "cpu", with = "humantime_serde")]
    pub cpu: Duration,
    /// The most rows one invocation may read and write together.
    #[serde(default = "rows")]
    pub rows: u64,
    /// How long a chain of calls runs before it is stopped, as a duration like
    /// `"60s"`.
    #[serde(default = "deadline", with = "humantime_serde")]
    pub deadline: Duration,
    /// How deep a chain of services calling services may go.
    #[serde(default = "depth")]
    pub depth: u32,
    /// Bytes of memory a scope's services may hold at once, their images and
    /// their invocations running, until its daemon limits it; omitted is no
    /// limit.
    #[serde(default)]
    pub held: Option<u64>,
    /// Where compiled code is cached between processes.
    #[serde(default)]
    pub cache_dir: Option<PathBuf>,
    /// Where the databases of the services that keep one live.
    #[serde(default = "databases")]
    pub databases: PathBuf,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            instances: instances(),
            memory: memory(),
            cpu: cpu(),
            rows: rows(),
            deadline: deadline(),
            depth: depth(),
            held: None,
            cache_dir: None,
            databases: databases(),
        }
    }
}

fn instances() -> u32 {
    berm::Config::default().instances
}

fn memory() -> usize {
    berm::Config::default().memory
}

fn deadline() -> Duration {
    berm::Config::default().deadline
}

fn cpu() -> Duration {
    berm::Config::default().cpu
}

fn rows() -> u64 {
    berm::Config::default().rows
}

fn depth() -> u32 {
    DEFAULT_CALL_DEPTH
}

fn databases() -> PathBuf {
    PathBuf::from("databases")
}
