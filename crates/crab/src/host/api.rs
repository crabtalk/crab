//! The control API: what a UI or a CLI drives the service with.
//!
//! Resource-shaped, on one endpoint, for the reason dockerd is: the clients are
//! a UI, a CLI and `curl`, and a program is a resource. The MCP endpoint that
//! agents reach sits on the same server at `/mcp`.

use crate::host::{Service, mcp::Mcp};
use axum::{
    Extension, Json, Router,
    body::Bytes,
    extract::{Path, Query, Request, State},
    http::StatusCode,
    middleware::{self, Next},
    response::Response,
    routing::{get, post, put},
};
use berm_api::{Deployed, Program as Wire, host};
use berm_host::{Denied, Named, deny};
use crabtalk_sdk::sandbox::service::{Secret, SetSecret};
use rmcp::transport::{
    StreamableHttpService, streamable_http_server::session::local::LocalSessionManager,
};
use serde::Deserialize;
use std::sync::Arc;

impl Service {
    /// The control API, at `/programs`, and MCP, at `/mcp`.
    pub fn router(self: Arc<Self>) -> Router {
        let service = self;
        // `route_service` rather than `nest_service`: MCP Streamable HTTP is one
        // path taking POST, GET and DELETE, and nesting can drop the `Host` header
        // the transport validates against.
        let mcp = StreamableHttpService::new(
            {
                let service = service.clone();
                move || Ok(Mcp::new(service.clone()))
            },
            Arc::new(LocalSessionManager::default()),
            Default::default(),
        );
        let program = berm_host::router(service.host.clone())
            .merge(
                Router::new()
                    .route("/", put(deploy).delete(undeploy))
                    .route("/start", post(start))
                    .route("/stop", post(stop))
                    .route("/secrets", get(secrets))
                    .route("/secrets/{secret}", put(set_secret).delete(revoke_secret))
                    .with_state(service.clone()),
            )
            .route_layer(middleware::from_fn(named));

        Router::new()
            .route("/programs", get(list))
            .with_state(service)
            .nest("/programs/{name}", program)
            .route_service("/mcp", mcp)
    }
}

/// The program a request names.
#[derive(Deserialize)]
struct Name {
    name: String,
}

/// Name the program a request is for, and the caller it is made as, from
/// [`host::CALLER`], unverified.
async fn named(
    Path(Name { name }): Path<Name>,
    mut request: Request,
    next: Next,
) -> Result<Response, Denied> {
    if let Some(value) = request.headers().get(host::CALLER) {
        let caller: host::Caller = serde_json::from_slice(value.as_bytes()).map_err(|e| {
            deny(
                StatusCode::BAD_REQUEST,
                format!("{} is not a caller: {e}", host::CALLER),
            )
        })?;
        request.extensions_mut().insert(caller);
    }
    request.extensions_mut().insert(Named(name));
    Ok(next.run(request).await)
}

async fn list(State(service): State<Arc<Service>>) -> Json<Vec<Wire>> {
    Json(
        service
            .list()
            .iter()
            .map(|one| one.idl(&one.name))
            .collect(),
    )
}

/// `?public=a,b&force=true&crons={"tick":"*/5 * * * *"}&size=1073741824&stopped=true`
/// on a deploy: the tools it may make public, whether to deploy over what it
/// would break or undo, each cron tool's expression, as a JSON object, when the
/// deploy sets them, the bytes its database may hold, and whether it installs
/// stopped.
#[derive(Deserialize)]
struct Confirmed {
    #[serde(default)]
    public: String,
    #[serde(default)]
    force: bool,
    crons: Option<String>,
    #[serde(default = "host::database_size")]
    size: u64,
    #[serde(default)]
    stopped: bool,
}

/// Deploy the image in the body under `name`. `PUT` because the name is the
/// address and redeploying the same name replaces what is there.
async fn deploy(
    State(service): State<Arc<Service>>,
    Extension(Named(name)): Extension<Named>,
    Query(confirmed): Query<Confirmed>,
    image: Bytes,
) -> Result<Json<Deployed>, Denied> {
    let public: Vec<String> = confirmed
        .public
        .split(',')
        .filter(|tool| !tool.is_empty())
        .map(str::to_owned)
        .collect();
    let crons = confirmed
        .crons
        .as_deref()
        .map(serde_json::from_str)
        .transpose()
        .map_err(|e| {
            deny(
                StatusCode::BAD_REQUEST,
                format!("`crons` is not a JSON object of tool to cron expression: {e}"),
            )
        })?;
    let (deployed, breaks) = service
        .deploy(
            &name,
            image.to_vec(),
            &public,
            confirmed.force,
            crons,
            confirmed.size,
            confirmed.stopped,
        )
        .await?;
    Ok(Json(Deployed {
        program: deployed.idl(&name),
        breaks: breaks.iter().map(ToString::to_string).collect(),
    }))
}

async fn undeploy(
    State(service): State<Arc<Service>>,
    Extension(Named(name)): Extension<Named>,
) -> Result<StatusCode, Denied> {
    found(service.undeploy(&name).await?, &name)
}

async fn start(
    State(service): State<Arc<Service>>,
    Extension(Named(name)): Extension<Named>,
) -> Result<StatusCode, Denied> {
    found(service.run(&name, true).await?, &name)
}

async fn stop(
    State(service): State<Arc<Service>>,
    Extension(Named(name)): Extension<Named>,
) -> Result<StatusCode, Denied> {
    found(service.run(&name, false).await?, &name)
}

/// `204` when the program `name` was deployed, else `404`.
fn found(deployed: bool, name: &str) -> Result<StatusCode, Denied> {
    match deployed {
        true => Ok(StatusCode::NO_CONTENT),
        false => Err(deny(
            StatusCode::NOT_FOUND,
            format!("no program named {name:?} is deployed"),
        )),
    }
}

/// The secret a path names, beside the program.
#[derive(Deserialize)]
struct SecretName {
    secret: String,
}

/// The program's secrets, by name; a value is never read back.
async fn secrets(
    State(service): State<Arc<Service>>,
    Extension(Named(name)): Extension<Named>,
) -> Result<Json<Vec<Secret>>, Denied> {
    service
        .get(&name)
        .ok_or_else(|| deny(StatusCode::NOT_FOUND, "no such program"))?;
    let listed = service.secrets.list(&name).map_err(internal)?;
    Ok(Json(
        listed
            .into_iter()
            .map(|(name, created_at)| Secret { name, created_at })
            .collect(),
    ))
}

/// Set one of the program's secrets. A set secret is changed only by revoking
/// it and setting it again.
async fn set_secret(
    State(service): State<Arc<Service>>,
    Extension(Named(name)): Extension<Named>,
    Path(SecretName { secret }): Path<SecretName>,
    Json(body): Json<SetSecret>,
) -> Result<StatusCode, Denied> {
    service
        .get(&name)
        .ok_or_else(|| deny(StatusCode::NOT_FOUND, "no such program"))?;
    if !Secret::valid(&secret) {
        return Err(deny(
            StatusCode::BAD_REQUEST,
            format!(
                "a secret's name is up to {} ASCII letters, digits and `_`, not starting with a digit",
                Secret::NAME_MAX
            ),
        ));
    }
    if !SetSecret::valid(&body.value) {
        return Err(deny(
            StatusCode::BAD_REQUEST,
            format!(
                "{secret}'s value is 1 to {} bytes on one line",
                SetSecret::VALUE_MAX
            ),
        ));
    }
    match service
        .secrets
        .set(&name, &secret, &body.value)
        .map_err(internal)?
    {
        true => Ok(StatusCode::NO_CONTENT),
        false => Err(deny(
            StatusCode::CONFLICT,
            format!("{secret} is set; revoke it to set it again"),
        )),
    }
}

/// Revoke one of the program's secrets.
async fn revoke_secret(
    State(service): State<Arc<Service>>,
    Extension(Named(name)): Extension<Named>,
    Path(SecretName { secret }): Path<SecretName>,
) -> Result<StatusCode, Denied> {
    match service.secrets.remove(&name, &secret).map_err(internal)? {
        true => Ok(StatusCode::NO_CONTENT),
        false => Err(deny(StatusCode::NOT_FOUND, "no secret of that name")),
    }
}

fn internal(error: anyhow::Error) -> Denied {
    deny(StatusCode::INTERNAL_SERVER_ERROR, format!("{error:#}"))
}
