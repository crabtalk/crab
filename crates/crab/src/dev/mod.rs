//! The local host's dev side: the workspaces it watches, each one's services
//! built, deployed and rebuilt as its files change, and what they do as
//! events, all at `/dev`.

use crate::host::Service;
use anyhow::{Result, bail};
use axum::{
    Json, Router,
    body::{Body, Bytes},
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post, put},
};
use berm_api::{Logged, host::Call, project::Project};
use berm_build::Workspace;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::{
    convert::Infallible,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
};
use tokio::sync::{broadcast, watch::Sender};

mod stream;
mod watch;

/// Events held for a reader that falls behind.
const BACKLOG: usize = 1024;

/// The host's dev state, as `GET /dev` answers it.
#[derive(Clone, Serialize, Deserialize)]
pub struct Status {
    pub pid: u32,
    pub root: PathBuf,
    /// When it started, in epoch milliseconds.
    pub since: u64,
    pub workspaces: Vec<Watching>,
}

impl Status {
    /// The `index`th service of the workspace watched at `root`.
    fn watched(&mut self, root: &Path, index: usize) -> Option<&mut Watched> {
        self.workspaces
            .iter_mut()
            .find(|w| w.root == root)?
            .services
            .get_mut(index)
    }
}

/// A workspace the host watches.
#[derive(Clone, Serialize, Deserialize)]
pub struct Watching {
    pub root: PathBuf,
    pub services: Vec<Watched>,
}

/// One service of a watched workspace.
#[derive(Clone, Serialize, Deserialize)]
pub struct Watched {
    pub name: String,
    pub dir: PathBuf,
    /// The version deployed, and when, in epoch milliseconds.
    pub version: Option<String>,
    pub deployed: Option<u64>,
    /// When it was last rebuilt, in epoch milliseconds, and why that failed.
    pub rebuilt: Option<u64>,
    pub error: Option<String>,
}

/// What a watched workspace does, as `GET /dev/events` streams it, one JSON
/// object a line.
#[derive(Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "lowercase")]
pub enum Event {
    Deployed {
        workspace: PathBuf,
        service: String,
        version: String,
        breaks: Vec<String>,
    },
    Failed {
        workspace: PathBuf,
        service: String,
        error: String,
    },
    Call {
        workspace: PathBuf,
        service: String,
        #[serde(flatten)]
        call: Call,
    },
    Log {
        workspace: PathBuf,
        service: String,
        #[serde(flatten)]
        log: Logged,
    },
}

impl Event {
    pub fn workspace(&self) -> &Path {
        match self {
            Event::Deployed { workspace, .. }
            | Event::Failed { workspace, .. }
            | Event::Call { workspace, .. }
            | Event::Log { workspace, .. } => workspace,
        }
    }

    /// The event as `crab dev` prints it.
    pub fn line(&self) -> String {
        match self {
            Event::Deployed {
                service,
                version,
                breaks,
                ..
            } => {
                let mut line = format!("deployed  {service} {version}");
                for broken in breaks {
                    line.push_str(&format!("\n  breaks  {broken}"));
                }
                line
            }
            Event::Failed { service, error, .. } => format!("failed    {service}: {error}"),
            Event::Call { service, call, .. } => format!(
                "call      {service}.{}  {}  {}ms",
                call.tool, call.outcome, call.ms
            ),
            Event::Log { log, .. } => format!("  log     {}  {}", log.topics.join(","), log.data),
        }
    }
}

pub struct Dev {
    pub service: Arc<Service>,
    status: Mutex<Status>,
    /// Each watched workspace's flag that stops its watch, by root.
    watches: Mutex<Vec<(PathBuf, Arc<AtomicBool>)>>,
    events: broadcast::Sender<Arc<str>>,
    /// Set once the host is asked to stop, which also ends every event
    /// stream, so the server can drain.
    stop: Sender<bool>,
}

impl Dev {
    pub fn new(service: Arc<Service>, status: Status) -> Arc<Self> {
        Arc::new(Self {
            service,
            status: Mutex::new(status),
            watches: Mutex::default(),
            events: broadcast::channel(BACKLOG).0,
            stop: Sender::new(false),
        })
    }

    /// The host's routes, with the dev routes beside them.
    pub fn router(self: Arc<Self>) -> Router {
        self.service.clone().router().merge(
            Router::new()
                .route("/dev", get(status))
                .route("/dev/workspaces", put(watch).delete(unwatch))
                .route("/dev/events", get(events))
                .route("/dev/stop", post(stop))
                .with_state(self),
        )
    }

    /// Ask the host to stop.
    pub fn stop(&self) {
        self.stop.send_replace(true);
    }

    /// Resolves once the host is asked to stop.
    pub async fn stopped(&self) {
        let _ = self.stop.subscribe().wait_for(|stopped| *stopped).await;
    }

    /// The root of the watched workspace `dir` is in.
    fn root_of(&self, dir: &Path) -> Option<PathBuf> {
        let dir = dir.canonicalize().ok()?;
        self.status
            .lock()
            .workspaces
            .iter()
            .map(|w| &w.root)
            .find(|root| dir.starts_with(root))
            .cloned()
    }

    pub fn status(&self) -> Status {
        self.status.lock().clone()
    }

    /// Watch the workspace `dir` is in, refused when one of its services is
    /// named as a service of another watched workspace. Watching it again
    /// changes nothing.
    pub fn watch_dir(self: &Arc<Self>, dir: &Path) -> Result<Watching> {
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
        let watching = Watching {
            root: workspace.root.clone(),
            services,
        };
        {
            let mut status = self.status.lock();
            if let Some(already) = status.workspaces.iter().find(|w| w.root == watching.root) {
                return Ok(already.clone());
            }
            for other in &status.workspaces {
                if let Some(taken) = watching
                    .services
                    .iter()
                    .find(|one| other.services.iter().any(|theirs| theirs.name == one.name))
                {
                    bail!(
                        "the service {} is already watched, from {}",
                        taken.name,
                        other.root.display()
                    );
                }
            }
            status.workspaces.push(watching.clone());
        }
        let stopped = Arc::new(AtomicBool::new(false));
        self.watches
            .lock()
            .push((workspace.root.clone(), stopped.clone()));
        let dev = self.clone();
        let root = workspace.root.clone();
        thread::spawn(move || {
            if let Err(error) = dev.clone().watch(workspace, stopped) {
                tracing::error!("watching {}: {error:#}", root.display());
                dev.unwatch(&root);
            }
        });
        Ok(watching)
    }

    /// Stop watching the workspace at `root`; its services stay deployed.
    /// `false` when it was not watched.
    pub fn unwatch(&self, root: &Path) -> bool {
        let mut watches = self.watches.lock();
        let Some(at) = watches.iter().position(|(watched, _)| watched == root) else {
            return false;
        };
        watches.remove(at).1.store(true, Ordering::Relaxed);
        drop(watches);
        self.status.lock().workspaces.retain(|w| w.root != root);
        true
    }

    /// Hand `event` to every reader, and print it to the host's own output.
    fn emit(&self, event: &Event) {
        if let Ok(line) = serde_json::to_string(event) {
            println!("{line}");
            let _ = self.events.send(line.into());
        }
    }
}

/// The workspace a dev request names, by its root or any directory in it.
#[derive(Serialize, Deserialize)]
pub struct Named {
    pub workspace: PathBuf,
}

async fn status(State(dev): State<Arc<Dev>>) -> Json<Status> {
    Json(dev.status())
}

async fn watch(State(dev): State<Arc<Dev>>, Json(named): Json<Named>) -> Response {
    let watched = tokio::task::spawn_blocking(move || dev.watch_dir(&named.workspace)).await;
    match watched {
        Ok(Ok(watching)) => Json(watching).into_response(),
        Ok(Err(error)) => (StatusCode::CONFLICT, format!("{error:#}")).into_response(),
        Err(error) => (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()).into_response(),
    }
}

async fn unwatch(State(dev): State<Arc<Dev>>, Query(named): Query<Named>) -> StatusCode {
    match dev.root_of(&named.workspace) {
        Some(root) if dev.unwatch(&root) => StatusCode::NO_CONTENT,
        _ => StatusCode::NOT_FOUND,
    }
}

/// The events of the workspace holding the directory named, from now on, one
/// JSON object a line, until the host stops. The workspace need not be
/// watched yet, so a reader can listen before asking for the watch.
async fn events(State(dev): State<Arc<Dev>>, Query(named): Query<Named>) -> Response {
    let Ok(dir) = named.workspace.canonicalize() else {
        return (StatusCode::NOT_FOUND, "no such directory").into_response();
    };
    let state = (dev.events.subscribe(), dev.stop.subscribe(), dir);
    let lines = futures_util::stream::unfold(state, |(mut events, mut stop, dir)| async move {
        loop {
            let line = tokio::select! {
                _ = stop.wait_for(|stopped| *stopped) => return None,
                line = events.recv() => line,
            };
            match line {
                Ok(line) => {
                    let ours = serde_json::from_str::<Event>(&line)
                        .is_ok_and(|event| dir.starts_with(event.workspace()));
                    if ours {
                        let bytes = Bytes::from(format!("{line}\n"));
                        return Some((Ok::<_, Infallible>(bytes), (events, stop, dir)));
                    }
                }
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(broadcast::error::RecvError::Closed) => return None,
            }
        }
    });
    Body::from_stream(lines).into_response()
}

async fn stop(State(dev): State<Arc<Dev>>) -> StatusCode {
    dev.stop();
    StatusCode::NO_CONTENT
}
