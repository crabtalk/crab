//! `crab dev` — the local host: deploy services to it and serve their tools
//! over HTTP and MCP, in the foreground until Ctrl-C or `crab dev stop`, or
//! in the background with `-d`. One per machine.

use crate::{
    context, format,
    host::{self, Dev, Service, Status, secrets},
    runtime, terminal,
};
use anyhow::{Context, Result, bail};
use berm::{Calls, clock};
use clap::Args;
use reqwest::StatusCode;
use std::{
    env,
    ffi::OsString,
    fs::{self, File},
    net::SocketAddr,
    path::{Path, PathBuf},
    process::{self, Command, Stdio},
    thread,
    time::{Duration, Instant},
};
use tokio::net::TcpListener;

/// The file the host started in the background writes to, under its root.
const LOG: &str = "host.log";

/// How often a host started in the background is asked whether it is up.
const POLL: Duration = Duration::from_millis(250);

/// How long a host started in the background has to answer.
const STARTING: Duration = Duration::from_secs(15);

/// How the host runs.
#[derive(Args)]
pub struct Daemon {
    /// Address to listen on. Loopback by default: the MCP endpoint carries
    /// no authorization yet, so anything wider would be an open one.
    #[arg(long, default_value = context::LOCAL)]
    pub addr: SocketAddr,
    /// Where deployed images and the code cache live; `~/.crab/host` by
    /// default.
    #[arg(long)]
    pub root: Option<PathBuf>,
    /// How deep a chain of services calling services may go. `0` refuses
    /// the first one, which is composition off.
    ///
    /// The bound is on runaway composition, not on the stack: a level
    /// costs ~720 bytes of it, and 64 MiB of guest address space.
    #[arg(long, default_value_t = Calls::DEFAULT_DEPTH)]
    pub max_call_depth: u32,
    /// The proxy `berm.http.proxy` goes through, e.g.
    /// `http://user:pass@gate.example.com:3120`. Without one it goes
    /// direct.
    #[arg(long)]
    pub proxy: Option<String>,
    /// How many milliseconds of CPU one call may spend, nested calls
    /// excluded.
    #[arg(long, default_value_t = berm::Config::default().cpu.as_millis() as u64)]
    pub max_cpu_millis: u64,
    /// How many seconds a call may run before it is stopped.
    #[arg(long, default_value_t = berm::Config::default().deadline.as_secs())]
    pub max_call_seconds: u64,
}

impl Daemon {
    fn root(&self) -> Result<PathBuf> {
        match &self.root {
            Some(root) => Ok(root.clone()),
            None => root(),
        }
    }

    /// The flags that run the same host again.
    fn args(&self) -> Vec<OsString> {
        let mut args: Vec<OsString> = vec![
            "--addr".into(),
            self.addr.to_string().into(),
            "--max-call-depth".into(),
            self.max_call_depth.to_string().into(),
            "--max-cpu-millis".into(),
            self.max_cpu_millis.to_string().into(),
            "--max-call-seconds".into(),
            self.max_call_seconds.to_string().into(),
        ];
        if let Some(root) = &self.root {
            args.extend(["--root".into(), root.into()]);
        }
        if let Some(proxy) = &self.proxy {
            args.extend(["--proxy".into(), proxy.into()]);
        }
        args
    }
}

/// `~/.crab/host`.
pub fn root() -> Result<PathBuf> {
    Ok(context::dir()?.join("host"))
}

pub fn url(path: &str) -> String {
    format!("http://{}{path}", context::LOCAL)
}

/// Run the host in the foreground until Ctrl-C or `crab dev stop`.
pub fn run(daemon: &Daemon) -> Result<()> {
    host::trace();
    let root = daemon.root()?;
    runtime::wait(async {
        let secrets = root.join(secrets::FILE);
        let service = Service::new(
            root.clone(),
            secrets,
            daemon.max_call_depth,
            Duration::from_millis(daemon.max_cpu_millis),
            Duration::from_secs(daemon.max_call_seconds),
            daemon.proxy.as_deref(),
        )
        .await?;
        let addr = daemon.addr;
        let listener = TcpListener::bind(addr)
            .await
            .with_context(|| format!("failed to bind {addr}"))?;
        let dev = Dev::new(
            service,
            Status {
                pid: process::id(),
                root,
                since: clock::now(),
            },
        );
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

/// Start the host in the background and return once it answers. A host
/// already answering is left as it is.
pub fn detach(daemon: &Daemon) -> Result<()> {
    let at = format!("http://{}", daemon.addr);
    match runtime::wait(ask(&at)) {
        Ok(Some(_)) => {
            eprintln!("the host is already running on {at}");
            Ok(())
        }
        Ok(None) => bail!("something other than the host answers on {at}"),
        Err(_) => start(&at, &daemon.root()?, daemon.args()),
    }
}

/// Make sure the host on [`context::LOCAL`] answers, starting it in the
/// background with its defaults when nothing does.
pub fn up() -> Result<()> {
    let at = url("");
    match runtime::wait(ask(&at)) {
        Ok(Some(_)) => Ok(()),
        Ok(None) => bail!("something other than the host answers on {at}"),
        Err(_) => start(&at, &root()?, Vec::new()),
    }
}

/// Run `crab dev` with `args` out of this terminal, writing to the log under
/// `root`, and wait until it answers at `at`.
fn start(at: &str, root: &Path, args: Vec<OsString>) -> Result<()> {
    fs::create_dir_all(root).with_context(|| format!("cannot create {}", root.display()))?;
    let path = root.join(LOG);
    let log = File::options()
        .create(true)
        .append(true)
        .open(&path)
        .with_context(|| format!("cannot write {}", path.display()))?;
    let mut command = Command::new(env::current_exe().context("cannot find crab itself")?);
    command
        .arg("dev")
        .args(args)
        .stdin(Stdio::null())
        .stdout(log.try_clone()?)
        .stderr(log);
    // Out of the terminal's process group, so its Ctrl-C and hangup pass it by.
    #[cfg(unix)]
    std::os::unix::process::CommandExt::process_group(&mut command, 0);
    #[cfg(windows)]
    std::os::windows::process::CommandExt::creation_flags(
        &mut command,
        DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP,
    );
    let mut child = command.spawn().context("cannot start the host")?;
    let started = Instant::now();
    loop {
        if let Some(exit) = child.try_wait()? {
            bail!("the host {exit}; {} says why", path.display());
        }
        if let Ok(Some(_)) = runtime::wait(ask(at)) {
            eprintln!("started the host on {at}, writing to {}", path.display());
            return Ok(());
        }
        if started.elapsed() > STARTING {
            bail!("the host did not answer; {} says why", path.display());
        }
        thread::sleep(POLL);
    }
}

#[cfg(windows)]
const DETACHED_PROCESS: u32 = 0x0000_0008;
#[cfg(windows)]
const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;

/// Print the host's state. Fails when no host answers.
pub fn status(json: bool) -> Result<()> {
    let status = match runtime::wait(ask(&url(""))) {
        Ok(Some(status)) => status,
        Ok(None) => bail!(
            "something other than the host answers on {}",
            context::LOCAL
        ),
        Err(_) => bail!("no host on {}", context::LOCAL),
    };
    if json {
        return terminal::json(&status);
    }
    println!(
        "host      http://{}  pid {}  since {}",
        context::LOCAL,
        status.pid,
        format::instant(status.since)
    );
    println!("root      {}", status.root.display());
    Ok(())
}

/// Stop the host.
pub fn stop() -> Result<()> {
    let response = runtime::wait(reqwest::Client::new().post(url("/dev/stop")).send())
        .with_context(|| format!("no host on {}", context::LOCAL))?;
    if response.status() == StatusCode::NOT_FOUND {
        bail!(
            "something other than the host answers on {}",
            context::LOCAL
        );
    }
    response.error_for_status()?;
    println!("stopped");
    Ok(())
}

/// The state of the host at `at`; `None` when something else answers there.
async fn ask(at: &str) -> reqwest::Result<Option<Status>> {
    let response = reqwest::get(format!("{at}/dev")).await?;
    if response.status() == StatusCode::NOT_FOUND {
        return Ok(None);
    }
    Ok(response.error_for_status()?.json().await.ok())
}
