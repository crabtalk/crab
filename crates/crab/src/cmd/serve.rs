//! `crab serve` — run the local host in the foreground until Ctrl-C or
//! `crab dev stop`: the one host on this machine, which `crab dev` hands
//! workspaces to watch.

use crate::{
    dev::{Dev, Status},
    host::{self, Service, secrets},
    utils,
};
use anyhow::{Context, Result};
use berm::syscall::call;
use std::{net::SocketAddr, path::PathBuf, process, time::Duration};
use tokio::net::TcpListener;

/// `~/.crab/host`.
pub fn root() -> Result<PathBuf> {
    Ok(utils::home()?.join(".crab").join("host"))
}

/// Serve on `addr` from `root`, or [`root`] without one. Chains of programs
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
        None => self::root()?,
    };
    utils::wait(async {
        let secrets = root.join(secrets::FILE);
        let service = Service::new(root.clone(), secrets, depth, cpu, deadline, proxy).await?;
        let listener = TcpListener::bind(addr)
            .await
            .with_context(|| format!("failed to bind {addr}"))?;
        let dev = Dev::new(
            service,
            Status {
                pid: process::id(),
                root,
                since: call::system_now(),
                workspaces: Vec::new(),
            },
        );
        tokio::spawn(dev.clone().stream());
        tracing::info!("listening on http://{addr}, mcp at http://{addr}/mcp");
        axum::serve(listener, dev.clone().router())
            .with_graceful_shutdown(async move {
                tokio::select! {
                    _ = tokio::signal::ctrl_c() => {}
                    _ = dev.stopped() => {}
                }
            })
            .await
            .context("server failed")
    })
}
