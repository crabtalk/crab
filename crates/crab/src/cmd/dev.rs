//! `crab dev` — have the local host watch a workspace: build and deploy its
//! services, and again as its files change. The host is the one on this
//! machine, started in the background when nothing answers.

use crate::{
    cmd::serve,
    context,
    dev::{Event, Named, Status, Watching},
    format, utils,
};
use anyhow::{Context, Result, bail};
use reqwest::StatusCode;
use std::{
    env,
    fs::{self, File},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

/// The file the host started in the background writes to, under its root.
const LOG: &str = "host.log";

/// How often the host is asked whether it is up, or a workspace deployed.
const POLL: Duration = Duration::from_millis(250);

/// How long a host started in the background has to answer.
const STARTING: Duration = Duration::from_secs(15);

fn url(path: &str) -> String {
    format!("http://{}{path}", context::LOCAL)
}

/// Watch the workspace `dir` is in and print what it does until Ctrl-C, which
/// stops the watch.
pub fn run(dir: &Path, json: bool) -> Result<()> {
    let dir = canonical(dir)?;
    up()?;
    let printed = utils::wait(async {
        // Listening before the watch begins, so its first deploys are heard.
        let mut response = reqwest::Client::new()
            .get(url("/dev/events"))
            .query(&Named {
                workspace: dir.clone(),
            })
            .send()
            .await?
            .error_for_status()?;
        let watching = watch(&dir).await?;
        eprintln!(
            "watching {} on http://{}; Ctrl-C stops watching it",
            watching.root.display(),
            context::LOCAL
        );
        for name in &watching.skipped {
            eprintln!("skipped   {name}: deploy = false");
        }
        let mut pending = Vec::new();
        loop {
            tokio::select! {
                _ = tokio::signal::ctrl_c() => return anyhow::Ok(()),
                chunk = response.chunk() => {
                    let Some(chunk) = chunk? else {
                        bail!("the host on {} stopped", context::LOCAL);
                    };
                    pending.extend_from_slice(&chunk);
                    while let Some(end) = pending.iter().position(|b| *b == b'\n') {
                        let line: Vec<u8> = pending.drain(..=end).collect();
                        let line = String::from_utf8_lossy(&line);
                        match json {
                            true => print!("{line}"),
                            false => {
                                if let Ok(event) = serde_json::from_str::<Event>(&line) {
                                    println!("{}", event.line());
                                }
                            }
                        }
                    }
                }
            }
        }
    });
    let _ = utils::wait(
        reqwest::Client::new()
            .delete(url("/dev/workspaces"))
            .query(&Named { workspace: dir })
            .send(),
    );
    printed
}

/// Watch the workspace `dir` is in, and return once each of its services is
/// deployed or failed.
pub fn detach(dir: &Path, json: bool) -> Result<()> {
    let dir = canonical(dir)?;
    up()?;
    let watching = utils::wait(watch(&dir))?;
    loop {
        let status = utils::wait(ask())?.context("the host stopped")?;
        let ours = status
            .workspaces
            .iter()
            .find(|w| w.root == watching.root)
            .context("the host stopped watching the workspace")?;
        if ours.services.iter().all(|one| one.rebuilt.is_some()) {
            return show(&status, Some(&watching.root), json);
        }
        thread::sleep(POLL);
    }
}

fn canonical(dir: &Path) -> Result<PathBuf> {
    dir.canonicalize()
        .with_context(|| format!("no directory {}", dir.display()))
}

/// Ask the host to watch the workspace `dir` is in.
async fn watch(dir: &Path) -> Result<Watching> {
    let response = reqwest::Client::new()
        .put(url("/dev/workspaces"))
        .json(&Named {
            workspace: dir.to_owned(),
        })
        .send()
        .await?;
    if !response.status().is_success() {
        bail!("{}", response.text().await?);
    }
    Ok(response.json().await?)
}

/// Make sure the host on this machine answers, starting it in the background
/// when nothing does.
fn up() -> Result<()> {
    match utils::wait(ask()) {
        Ok(Some(_)) => return Ok(()),
        Ok(None) => bail!(
            "the host on {} takes no workspaces; stop it and run `crab dev` again",
            context::LOCAL
        ),
        Err(_) => {}
    }
    let root = serve::root()?;
    fs::create_dir_all(&root).with_context(|| format!("cannot create {}", root.display()))?;
    let path = root.join(LOG);
    let log = File::options()
        .create(true)
        .append(true)
        .open(&path)
        .with_context(|| format!("cannot write {}", path.display()))?;
    let mut command = Command::new(env::current_exe().context("cannot find crab itself")?);
    command
        .arg("serve")
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
        if let Ok(Some(_)) = utils::wait(ask()) {
            eprintln!(
                "started the host on http://{}, writing to {}",
                context::LOCAL,
                path.display()
            );
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

/// Print the host's state and every workspace it watches. Fails when no
/// host answers.
pub fn status(json: bool) -> Result<()> {
    match utils::wait(ask()) {
        Ok(Some(status)) => show(&status, None, json),
        Ok(None) => bail!("the host on {} takes no workspaces", context::LOCAL),
        Err(_) => bail!("no host on {}", context::LOCAL),
    }
}

/// Stop the host.
pub fn stop() -> Result<()> {
    let response = utils::wait(reqwest::Client::new().post(url("/dev/stop")).send())
        .with_context(|| format!("no host on {}", context::LOCAL))?;
    if response.status() == StatusCode::NOT_FOUND {
        bail!("the host on {} takes no workspaces", context::LOCAL);
    }
    response.error_for_status()?;
    println!("stopped");
    Ok(())
}

/// The host's state, `None` from a host that takes no workspaces, such as
/// one from before the host was one per machine.
async fn ask() -> reqwest::Result<Option<Status>> {
    let response = reqwest::get(url("/dev")).await?;
    if response.status() == StatusCode::NOT_FOUND {
        return Ok(None);
    }
    Ok(response.error_for_status()?.json().await.ok())
}

/// Print `status`, its workspaces or only the one at `only`.
fn show(status: &Status, only: Option<&PathBuf>, json: bool) -> Result<()> {
    if json {
        return utils::json(status);
    }
    println!(
        "host      http://{}  pid {}  since {}",
        context::LOCAL,
        status.pid,
        format::instant(status.since)
    );
    println!("root      {}", status.root.display());
    let watched = status
        .workspaces
        .iter()
        .filter(|w| only.is_none_or(|root| w.root == *root));
    for watching in watched {
        println!("workspace {}", watching.root.display());
        for name in &watching.skipped {
            println!("  {name}  skipped: deploy = false");
        }
        for one in &watching.services {
            let deployed = match (&one.version, one.deployed) {
                (Some(version), Some(at)) => format!("{version}, deployed {}", format::instant(at)),
                _ => "not deployed".to_owned(),
            };
            println!("  {}  {deployed}", one.name);
            if let Some(error) = &one.error {
                println!("    failed  {error}");
            }
        }
    }
    Ok(())
}
