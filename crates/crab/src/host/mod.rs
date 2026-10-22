//! The local host — deploys programs and serves their tools over MCP.
//!
//! A thin daemon over [`Host`], run by `crab serve`, with one flat namespace of programs. Every
//! deployed program appears on one MCP endpoint, its tools namespaced
//! `{program}.{tool}`.

use anyhow::{Context, Result};
use berm::Program;
use berm_host::{Config, Edges, Host, Layers, System};
use std::{io::IsTerminal, path::PathBuf, sync::Arc, time::Duration};
use tokio::sync::broadcast;

pub use env::Local;
pub use fetch::Fetch;
pub use images::Stored;
pub use peers::Alone;
pub use secrets::Grants;

mod api;
mod env;
mod fetch;
mod images;
mod mcp;
mod peers;
mod program;
mod secrets;

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

/// How many deploys may go unread by a session before it misses one. A missed
/// notification costs a stale tool list until the next change, not a wrong one.
const CHANGE_BACKLOG: usize = 16;

pub struct Service {
    pub host: Host<Local>,
    /// Where uploaded images are kept for the host to pull.
    images: Stored,
    /// Fires when the deployed set changes. Connected MCP sessions turn this
    /// into `notifications/tools/list_changed`.
    changed: broadcast::Sender<()>,
}

impl Service {
    /// Open `root`, restoring whatever was deployed before this process.
    /// Chains of programs calling programs nest at most `depth` deep, and a
    /// call spends at most `cpu` of CPU and runs at most `deadline`.
    /// `berm.http.proxy` goes through `proxy`, or direct without one.
    pub async fn new(
        root: PathBuf,
        depth: u32,
        cpu: Duration,
        deadline: Duration,
        proxy: Option<&str>,
    ) -> Result<Arc<Self>> {
        let images = Stored::open(&root);
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
                fetch: Fetch::new(None)?,
                proxy: Some(Fetch::new(proxy)?),
                images: images.clone(),
                peers: Alone,
                secrets: Grants::open(&root),
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
            images,
            changed: broadcast::channel(CHANGE_BACKLOG).0,
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

    pub fn subscribe(&self) -> broadcast::Receiver<()> {
        self.changed.subscribe()
    }
}
