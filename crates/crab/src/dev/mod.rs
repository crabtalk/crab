//! `crab dev`'s host: the local host with a workspace's services built,
//! deployed and watched in its process, its state at `/dev`.

use crate::host::Service;
use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    routing::{get, post},
};
use berm_api::{Logged, host::Call};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::{path::PathBuf, sync::Arc};
use tokio::sync::Notify;

mod stream;
mod watch;

/// The dev host's state, as `GET /dev` answers it.
#[derive(Clone, Serialize, Deserialize)]
pub struct Status {
    pub pid: u32,
    pub root: PathBuf,
    pub workspace: PathBuf,
    /// When it started, in epoch milliseconds.
    pub since: u64,
    pub services: Vec<Watched>,
}

/// One service of the workspace.
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

/// What `crab dev` prints, one per line.
#[derive(Serialize)]
#[serde(tag = "event", rename_all = "lowercase")]
enum Event<'a> {
    Deployed {
        service: &'a str,
        version: &'a str,
        breaks: Vec<String>,
    },
    Failed {
        service: &'a str,
        error: &'a str,
    },
    Call {
        service: &'a str,
        #[serde(flatten)]
        call: &'a Call,
    },
    Log {
        service: &'a str,
        #[serde(flatten)]
        log: &'a Logged,
    },
}

pub struct Dev {
    pub service: Arc<Service>,
    status: Mutex<Status>,
    stop: Notify,
    /// Print events as JSON lines.
    json: bool,
}

impl Dev {
    pub fn new(service: Arc<Service>, status: Status, json: bool) -> Arc<Self> {
        Arc::new(Self {
            service,
            status: Mutex::new(status),
            stop: Notify::new(),
            json,
        })
    }

    /// The host's routes, with `GET /dev` and `POST /dev/stop`.
    pub fn router(self: Arc<Self>) -> Router {
        self.service.clone().router().merge(
            Router::new()
                .route("/dev", get(status))
                .route("/dev/stop", post(stop))
                .with_state(self),
        )
    }

    /// Ask the host to stop.
    pub fn stop(&self) {
        self.stop.notify_one();
    }

    /// Resolves once the host is asked to stop.
    pub async fn stopped(&self) {
        self.stop.notified().await
    }

    pub fn status(&self) -> Status {
        self.status.lock().clone()
    }

    fn emit(&self, event: &Event) {
        if self.json {
            if let Ok(line) = serde_json::to_string(event) {
                println!("{line}");
            }
            return;
        }
        match event {
            Event::Deployed {
                service,
                version,
                breaks,
            } => {
                println!("deployed  {service} {version}");
                for broken in breaks {
                    println!("  breaks  {broken}");
                }
            }
            Event::Failed { service, error } => println!("failed    {service}: {error}"),
            Event::Call { service, call } => println!(
                "call      {service}.{}  {}  {}ms",
                call.tool, call.outcome, call.ms
            ),
            Event::Log { log, .. } => println!("  log     {}  {}", log.topics.join(","), log.data),
        }
    }
}

async fn status(State(dev): State<Arc<Dev>>) -> Json<Status> {
    Json(dev.status())
}

async fn stop(State(dev): State<Arc<Dev>>) -> StatusCode {
    dev.stop();
    StatusCode::NO_CONTENT
}
