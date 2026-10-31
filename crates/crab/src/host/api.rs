//! The control API: what a UI or a CLI drives the service with.
//!
//! Resource-shaped, on one endpoint, for the reason dockerd is: the clients are
//! a UI, a CLI and `curl`, and a service is a resource. The MCP endpoint that
//! agents reach sits on the same server at `/mcp`.

use crate::host::{Node, mcp::Mcp};
use axum::{
    Extension, Json, Router,
    body::Bytes,
    extract::{Path, Request, State},
    http::{HeaderMap, StatusCode, header},
    middleware::{self, Next},
    response::Response,
    routing::{get, post, put},
};
use berm_api::{Deployed, Service as Wire, digest, host};
use berm_host::{Denied, Named, deny};
use crabtalk_sdk::{
    client::LOCAL,
    services::service::{
        DeployRequest, DeployResult, DeploySetRequest, Hosts, Secret, SetSecret, Upload,
        UploadRequest,
    },
};
use rmcp::transport::{
    StreamableHttpService, streamable_http_server::session::local::LocalSessionManager,
};
use serde::Deserialize;
use std::{collections::BTreeMap, sync::Arc};

impl Node {
    /// The control API, at Cloud's paths for the one account [`LOCAL`], and
    /// MCP, at `/mcp`.
    pub fn router(self: Arc<Self>) -> Router {
        let node = self;
        // MCP Streamable HTTP is one path taking POST, GET and DELETE, and
        // `nest_service` can drop the `Host` header the transport validates.
        let mcp = StreamableHttpService::new(
            {
                let node = node.clone();
                move || Ok(Mcp::new(node.clone()))
            },
            Arc::new(LocalSessionManager::default()),
            Default::default(),
        );
        let service = berm_host::router(node.host.clone())
            .merge(
                Router::new()
                    .route("/", put(deploy).delete(undeploy))
                    .route("/image", post(image))
                    .route("/image/{sha256}", put(upload))
                    .route("/start", post(start))
                    .route("/stop", post(stop))
                    .route("/secrets", get(secrets))
                    .route("/secrets/{secret}", put(set_secret).delete(revoke_secret))
                    .route("/secrets/{secret}/hosts", put(set_secret_hosts))
                    .with_state(node.clone()),
            )
            .route_layer(middleware::from_fn(named));

        Router::new()
            .route(&format!("/users/{LOCAL}/services"), get(list))
            .route(
                &format!("/users/{LOCAL}/services/deploys"),
                post(deploy_set),
            )
            .with_state(node)
            .nest(&format!("/services/{LOCAL}/{{name}}"), service)
            .route_service("/mcp", mcp)
    }
}

/// The service a request names.
#[derive(Deserialize)]
struct Name {
    name: String,
}

/// Name the service a request is for, and the caller it is made as, from
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

async fn list(State(node): State<Arc<Node>>) -> Json<Vec<Wire>> {
    Json(node.list().iter().map(|one| one.idl(&one.name)).collect())
}

/// Deploy under `name` the image uploaded through
/// `POST /services/local/{name}/image` that the body names by its digest.
/// Redeploying the same name replaces what is there.
async fn deploy(
    State(node): State<Arc<Node>>,
    Extension(Named(name)): Extension<Named>,
    Json(deploy): Json<DeployRequest>,
) -> Result<Json<Deployed>, Denied> {
    let priced: Vec<String> = deploy.prices.keys().cloned().collect();
    let deploy = kept(&name, deploy)?;
    node.priced(&deploy.digest, &priced).await?;
    let (deployed, breaks) = node.deploy(&name, deploy).await?;
    Ok(Json(Deployed {
        service: deployed.service.idl(&name),
        breaks: breaks.iter().map(ToString::to_string).collect(),
        migrations: deployed.migrations,
        page: None,
    }))
}

/// `deploy` of `name` as the host runs it, refused when it names a seed or an
/// image not uploaded through `POST /services/local/{name}/image`.
fn kept(name: &str, deploy: DeployRequest) -> Result<host::Deploy, Denied> {
    if deploy.seed.is_some() {
        return Err(deny(
            StatusCode::BAD_REQUEST,
            "a seed starts a service on Cloud; the local host's own database is where one is made",
        ));
    }
    if !deploy.reference.starts_with(digest::PREFIX) {
        return Err(deny(
            StatusCode::BAD_REQUEST,
            format!(
                "{name}: the local host deploys an image uploaded through POST /services/local/{name}/image, by its digest"
            ),
        ));
    }
    Ok(host::Deploy {
        digest: deploy.reference.clone(),
        reference: deploy.reference,
        force: deploy.force,
        crons: deploy.crons,
        size: deploy.size.unwrap_or_else(host::database_size),
        stopped: deploy.stopped,
        check_secrets: true,
        project: deploy.project,
        exempt: Vec::new(),
        vars: deploy.vars,
    })
}

/// Where to `PUT` the image `upload` describes for a deploy of `name` to name
/// by its digest, `sha256:` and the `sha256` here: none when it is kept
/// already.
async fn image(
    State(node): State<Arc<Node>>,
    Extension(Named(name)): Extension<Named>,
    headers: HeaderMap,
    Json(upload): Json<UploadRequest>,
) -> Result<Json<Upload>, Denied> {
    if upload.sha256.len() != 64
        || !upload
            .sha256
            .chars()
            .all(|c| matches!(c, '0'..='9' | 'a'..='f'))
    {
        return Err(deny(
            StatusCode::BAD_REQUEST,
            "`sha256` is a SHA-256 in lowercase hex",
        ));
    }
    if node.holds(&digest::prefixed(&upload.sha256)).await {
        return Ok(Json(Upload { upload: None }));
    }
    let host = headers
        .get(header::HOST)
        .and_then(|host| host.to_str().ok())
        .ok_or_else(|| deny(StatusCode::BAD_REQUEST, "no Host header"))?;
    Ok(Json(Upload {
        upload: Some(format!(
            "http://{host}/services/{LOCAL}/{name}/image/{}",
            upload.sha256
        )),
    }))
}

/// Keep the image in the body, refused when its SHA-256 is not `sha256`.
async fn upload(
    State(node): State<Arc<Node>>,
    Path((_, sha256)): Path<(String, String)>,
    image: Bytes,
) -> Result<StatusCode, Denied> {
    let kept = node.keep(&image).await?;
    let named = digest::prefixed(&sha256);
    if kept != named {
        return Err(deny(
            StatusCode::BAD_REQUEST,
            format!("the image uploaded is {kept}, not {named}"),
        ));
    }
    Ok(StatusCode::NO_CONTENT)
}

/// Deploy a set of services together, each named by the digest of an image
/// uploaded through `POST /services/local/{name}/image`.
async fn deploy_set(
    State(node): State<Arc<Node>>,
    Json(set): Json<DeploySetRequest>,
) -> Result<Json<BTreeMap<String, DeployResult>>, Denied> {
    let mut members = Vec::new();
    let mut results = BTreeMap::new();
    for (name, request) in set.services {
        let prepared = async {
            super::service::validate(&name)?;
            let priced = request.prices.keys().cloned().collect::<Vec<_>>();
            let deploy = kept(&name, request)?;
            node.priced(&deploy.digest, &priced).await?;
            Ok::<_, Denied>(deploy)
        }
        .await;
        match prepared {
            Ok(deploy) => members.push((name, deploy)),
            Err(denied) if set.partial => {
                results.insert(
                    name,
                    DeployResult::Failed {
                        failure: denied.message,
                        explain: denied.explain.map(str::to_owned),
                    },
                );
            }
            Err(denied) => return Err(denied),
        }
    }
    if set.partial {
        let replaced = members
            .iter()
            .map(|(name, _)| (name.clone(), node.get(name)))
            .collect::<BTreeMap<_, _>>();
        for (name, result) in node.host.deploy_partial(members).await? {
            let result = match result {
                Ok(installed) => {
                    let breaks = replaced
                        .get(&name)
                        .and_then(Option::as_ref)
                        .map(|old| old.manifest.breaks(&installed.service.manifest))
                        .unwrap_or_default();
                    DeployResult::from(Deployed {
                        service: installed.service.idl(&name),
                        breaks: breaks.iter().map(ToString::to_string).collect(),
                        migrations: installed.migrations,
                        page: None,
                    })
                }
                Err(denied) => DeployResult::Failed {
                    failure: denied.message,
                    explain: denied.explain.map(str::to_owned),
                },
            };
            results.insert(name, result);
        }
    } else {
        for (name, installed, breaks) in node.deploy_set(members).await? {
            results.insert(
                name.clone(),
                DeployResult::from(Deployed {
                    service: installed.service.idl(&name),
                    breaks: breaks.iter().map(ToString::to_string).collect(),
                    migrations: installed.migrations,
                    page: None,
                }),
            );
        }
    }
    Ok(Json(results))
}

async fn undeploy(
    State(node): State<Arc<Node>>,
    Extension(Named(name)): Extension<Named>,
) -> Result<StatusCode, Denied> {
    found(node.undeploy(&name).await?, &name)
}

async fn start(
    State(node): State<Arc<Node>>,
    Extension(Named(name)): Extension<Named>,
) -> Result<StatusCode, Denied> {
    found(node.run(&name, true).await?, &name)
}

async fn stop(
    State(node): State<Arc<Node>>,
    Extension(Named(name)): Extension<Named>,
) -> Result<StatusCode, Denied> {
    found(node.run(&name, false).await?, &name)
}

/// `204` when the service `name` was deployed, else `404`.
fn found(deployed: bool, name: &str) -> Result<StatusCode, Denied> {
    match deployed {
        true => Ok(StatusCode::NO_CONTENT),
        false => Err(deny(
            StatusCode::NOT_FOUND,
            format!("no service named {name:?} is deployed"),
        )),
    }
}

/// The secret a path names, beside the service.
#[derive(Deserialize)]
struct SecretName {
    secret: String,
}

/// The service's secrets, by name; a value is never read back.
async fn secrets(
    State(node): State<Arc<Node>>,
    Extension(Named(name)): Extension<Named>,
) -> Result<Json<Vec<Secret>>, Denied> {
    node.get(&name)
        .ok_or_else(|| deny(StatusCode::NOT_FOUND, "no such service"))?;
    let listed = node.secrets.list(&name).map_err(internal)?;
    Ok(Json(
        listed
            .into_iter()
            .map(|(name, hosts, created_at)| Secret {
                name,
                hosts,
                created_at,
            })
            .collect(),
    ))
}

/// Set one of the service's secrets. A set secret's value is changed only by
/// revoking it and setting it again.
async fn set_secret(
    State(node): State<Arc<Node>>,
    Extension(Named(name)): Extension<Named>,
    Path(SecretName { secret }): Path<SecretName>,
    Json(body): Json<SetSecret>,
) -> Result<StatusCode, Denied> {
    let service = node
        .get(&name)
        .ok_or_else(|| deny(StatusCode::NOT_FOUND, "no such service"))?;
    body.check(&secret)
        .map_err(|why| deny(StatusCode::BAD_REQUEST, why))?;
    service
        .manifest
        .deps
        .admits(&secret, &body.value)
        .map_err(|why| deny(StatusCode::BAD_REQUEST, why))?;
    match node
        .secrets
        .set(&name, &secret, &body.value, &body.hosts)
        .map_err(internal)?
    {
        true => Ok(StatusCode::NO_CONTENT),
        false => Err(deny(
            StatusCode::CONFLICT,
            format!("{secret} is set; revoke it to set it again"),
        )),
    }
}

/// Send one of the service's secrets to these hosts alone.
async fn set_secret_hosts(
    State(node): State<Arc<Node>>,
    Extension(Named(name)): Extension<Named>,
    Path(SecretName { secret }): Path<SecretName>,
    Json(body): Json<Hosts>,
) -> Result<StatusCode, Denied> {
    Hosts::check(&body.hosts).map_err(|why| deny(StatusCode::BAD_REQUEST, why))?;
    match node
        .secrets
        .set_hosts(&name, &secret, &body.hosts)
        .map_err(internal)?
    {
        true => Ok(StatusCode::NO_CONTENT),
        false => Err(deny(StatusCode::NOT_FOUND, "no secret of that name")),
    }
}

/// Revoke one of the service's secrets.
async fn revoke_secret(
    State(node): State<Arc<Node>>,
    Extension(Named(name)): Extension<Named>,
    Path(SecretName { secret }): Path<SecretName>,
) -> Result<StatusCode, Denied> {
    match node.secrets.remove(&name, &secret).map_err(internal)? {
        true => Ok(StatusCode::NO_CONTENT),
        false => Err(deny(StatusCode::NOT_FOUND, "no secret of that name")),
    }
}

fn internal(error: anyhow::Error) -> Denied {
    deny(StatusCode::INTERNAL_SERVER_ERROR, format!("{error:#}"))
}
