//! The local host's own routes, at `/dev`: what is running, and stopping it.

use crate::host::Service;
use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use std::{path::PathBuf, sync::Arc};
use tokio::sync::watch::Sender;

/// The running host, as `GET /dev` answers it.
#[derive(Clone, Serialize, Deserialize)]
pub struct Status {
    pub pid: u32,
    pub root: PathBuf,
    /// When it started, in epoch milliseconds.
    pub since: u64,
}

pub struct Dev {
    pub service: Arc<Service>,
    pub status: Status,
    /// Set once the host is asked to stop, so the server can drain.
    stop: Sender<bool>,
}

impl Dev {
    pub fn new(service: Arc<Service>, status: Status) -> Arc<Self> {
        Arc::new(Self {
            service,
            status,
            stop: Sender::new(false),
        })
    }

    /// The host's routes, with the dev routes beside them.
    pub fn router(self: Arc<Self>) -> Router {
        self.service.clone().router().merge(
            Router::new()
                .route("/dev", get(status))
                .route("/dev/stop", post(stop))
                .with_state(self),
        )
    }

    /// Resolves once the host is asked to stop.
    pub async fn stopped(&self) {
        let _ = self.stop.subscribe().wait_for(|stopped| *stopped).await;
    }
}

async fn status(State(dev): State<Arc<Dev>>) -> Json<Status> {
    Json(dev.status.clone())
}

async fn stop(State(dev): State<Arc<Dev>>) -> StatusCode {
    dev.stop.send_replace(true);
    StatusCode::NO_CONTENT
}
