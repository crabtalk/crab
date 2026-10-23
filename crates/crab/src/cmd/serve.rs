//! `crab serve` — run the local host in the foreground until Ctrl-C.

use crate::{
    host::{self, Service, secrets},
    utils,
};
use anyhow::{Context, Result};
use std::{net::SocketAddr, path::PathBuf, time::Duration};
use tokio::net::TcpListener;

/// Serve on `addr` from `root`, or `~/.berm` without one. Chains of programs
/// calling programs nest at most `depth` deep, and a call spends at most `cpu`
/// of CPU and runs at most `deadline`. `berm.http.proxy` goes through `proxy`,
/// or direct without one.
pub fn run(
    addr: SocketAddr,
    root: Option<PathBuf>,
    depth: u32,
    cpu: Duration,
    deadline: Duration,
    proxy: Option<&str>,
) -> Result<()> {
    host::trace();
    let root = match root {
        Some(root) => root,
        None => utils::home()?.join(".berm"),
    };
    utils::wait(async {
        let secrets = root.join(secrets::FILE);
        let service = Service::new(root, secrets, depth, cpu, deadline, proxy).await?;
        let listener = TcpListener::bind(addr)
            .await
            .with_context(|| format!("failed to bind {addr}"))?;
        tracing::info!("listening on http://{addr}, mcp at http://{addr}/mcp");
        axum::serve(listener, service.router())
            .with_graceful_shutdown(async {
                let _ = tokio::signal::ctrl_c().await;
            })
            .await
            .context("server failed")
    })
}
