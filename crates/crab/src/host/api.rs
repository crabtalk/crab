//! The control API: what a UI or a CLI drives the service with.
//!
//! Resource-shaped, on one endpoint, for the reason dockerd is: the clients are
//! a UI, a CLI and `curl`, and a program is a resource. The MCP endpoint that
//! agents reach sits on the same server at `/mcp`.

use crate::host::{Service, mcp::Mcp};
use axum::{
    Extension, Json, Router,
    body::Bytes,
    extract::{Path, Request, State},
    http::StatusCode,
    middleware::{self, Next},
    response::Response,
    routing::{get, post, put},
};
use berm_api::{Deployed, Program as Wire, host};
use berm_host::{Denied, Named, deny};
use crabtalk_sdk::sandbox::service::{DeployRequest, DeploySetRequest, Secret, SetSecret};
use rmcp::transport::{
    StreamableHttpService, streamable_http_server::session::local::LocalSessionManager,
};
use serde::Deserialize;
use std::{collections::BTreeMap, sync::Arc};

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
            .route("/images", post(keep))
            .route("/deploys", post(deploy_set))
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

/// Deploy under `name` the image kept through `POST /images` that the body
/// names by its digest. `PUT` because the name is the address and redeploying
/// the same name replaces what is there.
async fn deploy(
    State(service): State<Arc<Service>>,
    Extension(Named(name)): Extension<Named>,
    Json(deploy): Json<DeployRequest>,
) -> Result<Json<Deployed>, Denied> {
    let (deployed, breaks) = service.deploy(&name, kept(&name, deploy)?).await?;
    Ok(Json(Deployed {
        program: deployed.program.idl(&name),
        breaks: breaks.iter().map(ToString::to_string).collect(),
        migrations: deployed.migrations,
    }))
}

/// `deploy` of `name` as the host runs it, refused when it names a seed or an
/// image not kept through `POST /images`.
fn kept(name: &str, deploy: DeployRequest) -> Result<host::Deploy, Denied> {
    if deploy.seed.is_some() {
        return Err(deny(
            StatusCode::BAD_REQUEST,
            "a seed starts a service on Cloud; the local host's own database is where one is made",
        ));
    }
    if !deploy.reference.starts_with("sha256:") {
        return Err(deny(
            StatusCode::BAD_REQUEST,
            format!(
                "{name}: the local host deploys an image kept through POST /images, by its digest"
            ),
        ));
    }
    Ok(host::Deploy {
        digest: deploy.reference.clone(),
        reference: deploy.reference,
        force: deploy.force,
        crons: deploy.crons,
        size: deploy.size,
        stopped: deploy.stopped,
        project: deploy.project,
        exempt: Vec::new(),
    })
}

/// Keep the image in the body for a deploy to name, answering its digest.
async fn keep(State(service): State<Arc<Service>>, image: Bytes) -> Result<String, Denied> {
    service.keep(&image).await
}

/// Deploy a set of programs together, each named by the digest of an image
/// kept through `POST /images`.
async fn deploy_set(
    State(service): State<Arc<Service>>,
    Json(set): Json<DeploySetRequest>,
) -> Result<Json<BTreeMap<String, Deployed>>, Denied> {
    let mut members = Vec::new();
    for (name, deploy) in set.services {
        let deploy = kept(&name, deploy)?;
        members.push((name, deploy));
    }
    Ok(Json(
        service
            .deploy_set(members)
            .await?
            .into_iter()
            .map(|(name, installed, breaks)| {
                let deployed = Deployed {
                    program: installed.program.idl(&name),
                    breaks: breaks.iter().map(ToString::to_string).collect(),
                    migrations: installed.migrations,
                };
                (name, deployed)
            })
            .collect(),
    ))
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
