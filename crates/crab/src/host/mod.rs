//! The local host — deploys programs and serves their tools over MCP.
//!
//! A thin daemon over [`Host`], run by `crab dev`, with one flat namespace of programs, every
//! one reached through one MCP endpoint.

use crate::host::secrets::Grants;
use anyhow::{Context, Result};
use berm::Program;
use berm_host::{Alone, Config, Edges, Host, Layers, Stored, System};
use std::{io::IsTerminal, path::PathBuf, sync::Arc, time::Duration};

pub use dev::{Dev, Status};
pub use env::Local;
pub use fetch::Fetch;

mod api;
mod dev;
mod env;
mod fetch;
mod mcp;
mod program;
pub mod secrets;

/// Write the host's log lines to stderr, `info` and up of `crab` and `berm` unless
/// `RUST_LOG` says otherwise.
pub fn trace() {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_ansi(std::io::stderr().is_terminal())
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(
                    "crab_agent=info"
                        .parse()
                        .expect("the default directive parses"),
                )
                // The runtime restores its own images, so its target has to
                // be on by default or coming back says nothing.
                .add_directive("berm=info".parse().expect("the default directive parses")),
        )
        .init();
}

pub struct Service {
    pub host: Host<Local>,
    /// Each service's secrets, which the host reads and the control API sets.
    pub secrets: Grants,
    /// Where uploaded images are kept for the host to pull.
    images: Stored,
}

impl Service {
    /// Open `root`, restoring whatever was deployed before this process, its
    /// programs' secrets read from the file at `secrets`. Chains of programs
    /// calling programs nest at most `depth` deep, and a call spends at most
    /// `cpu` of CPU and runs at most `deadline`. `berm.http.proxy` goes
    /// through `proxy`, or direct without one.
    pub async fn new(
        root: PathBuf,
        secrets: PathBuf,
        depth: u32,
        cpu: Duration,
        deadline: Duration,
        proxy: Option<&str>,
    ) -> Result<Arc<Self>> {
        let images = Stored::open(&root);
        let grants = Grants::open(secrets);
        let fetch = Fetch::new(None)?;
        let proxy = match proxy {
            Some(proxy) => Fetch::new(Some(proxy))?,
            None => fetch.clone(),
        };
        let config = Config {
            depth,
            cpu,
            deadline,
            cache_dir: Some(root.join("cache")),
            databases: root.join("databases"),
            ..Config::default()
        };
        let host = Host::open(
            &root.join("host.db"),
            &config,
            Edges {
                fetch,
                proxy: Some(proxy),
                images: images.clone(),
                peers: Alone,
                secrets: grants.clone(),
                layers: Layers::new(tower::layer::util::Identity::new()),
                observe: (),
                clock: System,
            },
        )
        .await
        .context("failed to open the host")?;
        host.boot();
        Ok(Arc::new(Self {
            host,
            secrets: grants,
            images,
        }))
    }

    /// Every deployed program, as a snapshot the caller can hold across awaits.
    pub fn list(&self) -> Vec<Arc<Program>> {
        let mut programs: Vec<Arc<Program>> = self
            .host
            .list("")
            .into_iter()
            .map(|(_, program)| program)
            .collect();
        programs.sort_by(|a, b| a.name.cmp(&b.name));
        programs
    }

    pub fn get(&self, name: &str) -> Option<Arc<Program>> {
        self.host.get(name)
    }
}
