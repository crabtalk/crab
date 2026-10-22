//! `crab dev` — run the local host with the workspace's services built,
//! deployed and watched, until Ctrl-C or `crab dev stop`.

use crate::{
    context::{self, Contexts},
    dev::{Dev, Status, Watched},
    format, host,
    host::Service,
    utils,
};
use anyhow::{Context, Result, bail};
use berm::syscall::call;
use berm_api::project::Project;
use berm_build::Workspace;
use reqwest::StatusCode;
use std::{
    env,
    fs::{self, File},
    path::{Path, PathBuf},
    process::{self, Command, Stdio},
    thread,
    time::Duration,
};
use tokio::net::TcpListener;

/// The file a detached dev host writes to, under its root.
const LOG: &str = "dev.log";

/// How often `--detach` asks whether the first deploys are done.
const POLL: Duration = Duration::from_millis(250);

/// `~/.crab/dev`.
fn root() -> Result<PathBuf> {
    Ok(utils::home()?.join(".crab").join("dev"))
}

/// Run the dev host for the services `dir` names in the foreground.
pub fn run(dir: &Path, json: bool) -> Result<()> {
    host::trace();
    let workspace = Workspace::read(dir)?;
    if workspace.services.is_empty() {
        bail!(
            "no {} in the workspace at {} names a service",
            Project::FILE,
            workspace.root.display()
        );
    }
    let mut services = Vec::new();
    for dir in &workspace.services {
        services.push(Watched {
            name: Project::read(dir)?.name()?.to_owned(),
            dir: dir.clone(),
            version: None,
            deployed: None,
            rebuilt: None,
            error: None,
        });
    }
    let root = root()?;
    if let Some(current) = Contexts::load()?.current
        && current != context::DEFAULT
    {
        eprintln!(
            "the context in use is {current}; `crab --context {} …` reaches this host",
            context::DEFAULT
        );
    }

    let config = berm::Config::default();
    utils::wait(async {
        let service = Service::new(
            root.clone(),
            call::DEFAULT_CALL_DEPTH,
            config.cpu,
            config.deadline,
            None,
        )
        .await?;
        let listener = TcpListener::bind(context::LOCAL).await.with_context(|| {
            format!(
                "{} is taken; `crab dev status` says whether a dev host holds it",
                context::LOCAL
            )
        })?;
        let status = Status {
            pid: process::id(),
            root,
            workspace: workspace.root.clone(),
            since: call::system_now(),
            services,
        };
        let dev = Dev::new(service, status, json);
        tracing::info!("dev host on http://{}", context::LOCAL);

        let watching = dev.clone();
        thread::spawn(move || {
            if let Err(error) = watching.clone().watch(workspace) {
                eprintln!("crab dev: {error:#}");
                watching.stop();
            }
        });
        tokio::spawn(dev.clone().stream());
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

/// Run the dev host for `dir` in the background, and return once each
/// service's first deploy is done or failed.
pub fn detach(dir: &Path, json: bool) -> Result<()> {
    let dir = dir
        .canonicalize()
        .with_context(|| format!("no directory {}", dir.display()))?;
    let root = root()?;
    fs::create_dir_all(&root).with_context(|| format!("cannot create {}", root.display()))?;
    let path = root.join(LOG);
    let log = File::create(&path).with_context(|| format!("cannot write {}", path.display()))?;
    let mut command = Command::new(env::current_exe().context("cannot find crab itself")?);
    command
        .arg("dev")
        .arg(&dir)
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
    let mut child = command.spawn().context("cannot start crab dev")?;

    loop {
        if let Some(exit) = child.try_wait()? {
            bail!("crab dev {exit}; {} says why", path.display());
        }
        if let Ok(Some(status)) = utils::wait(ask())
            && status.pid == child.id()
            && status.services.iter().all(|one| one.rebuilt.is_some())
        {
            return show(&status, json);
        }
        thread::sleep(POLL);
    }
}

#[cfg(windows)]
const DETACHED_PROCESS: u32 = 0x0000_0008;
#[cfg(windows)]
const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;

/// Print the dev host's state. Fails when no dev host answers.
pub fn status(json: bool) -> Result<()> {
    match utils::wait(ask()) {
        Ok(Some(status)) => show(&status, json),
        Ok(None) => bail!("the host on {} is not a dev host", context::LOCAL),
        Err(_) => bail!("no dev host on {}", context::LOCAL),
    }
}

/// Stop the dev host.
pub fn stop() -> Result<()> {
    let url = format!("http://{}/dev/stop", context::LOCAL);
    let response = utils::wait(reqwest::Client::new().post(url).send())
        .with_context(|| format!("no dev host on {}", context::LOCAL))?;
    if response.status() == StatusCode::NOT_FOUND {
        bail!("the host on {} is not a dev host", context::LOCAL);
    }
    response.error_for_status()?;
    println!("stopped");
    Ok(())
}

/// The dev host's state, `None` from a host that is not one.
async fn ask() -> reqwest::Result<Option<Status>> {
    let response = reqwest::get(format!("http://{}/dev", context::LOCAL)).await?;
    if response.status() == StatusCode::NOT_FOUND {
        return Ok(None);
    }
    Ok(Some(response.error_for_status()?.json().await?))
}

fn show(status: &Status, json: bool) -> Result<()> {
    if json {
        return utils::json(status);
    }
    println!(
        "dev host  http://{}  pid {}  since {}",
        context::LOCAL,
        status.pid,
        format::instant(status.since)
    );
    println!("root      {}", status.root.display());
    println!("workspace {}", status.workspace.display());
    for watched in &status.services {
        let deployed = match (&watched.version, watched.deployed) {
            (Some(version), Some(at)) => format!("{version}, deployed {}", format::instant(at)),
            _ => "not deployed".to_owned(),
        };
        println!("  {}  {deployed}", watched.name);
        if let Some(error) = &watched.error {
            println!("    failed  {error}");
        }
    }
    Ok(())
}
