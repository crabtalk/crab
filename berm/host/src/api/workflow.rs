use crate::{Denied, api::serve::Named, env::Env, host::Host, workflow::Intent};
use axum::{
    Extension, Json, Router,
    extract::{Path, Request, State},
    http::{StatusCode, header},
    middleware,
    response::Response,
    routing::{get, post},
};
use berm_api::{
    host::Caller,
    workflow::view::{Inspection, Record, Start},
};

pub(crate) fn router<E: Env>() -> Router<Host<E>> {
    Router::new()
        .route("/workflows/{workflow}", get(inspect::<E>))
        .route("/workflows/{workflow}/runs", post(start::<E>))
        .route("/workflows/{workflow}/runs/{run}", get(read::<E>))
        .route(
            "/workflows/{workflow}/runs/{run}/continue",
            post(continue_run::<E>),
        )
        .route("/workflows/{workflow}/runs/{run}/cancel", post(cancel::<E>))
        .route_layer(middleware::from_fn(no_store))
}

async fn inspect<E: Env>(
    State(host): State<Host<E>>,
    Extension(Named(service)): Extension<Named>,
    Path(workflow): Path<String>,
    caller: Option<Extension<Caller>>,
) -> Result<Json<Inspection>, Denied> {
    Ok(Json(host.inspect_workflow(
        &service,
        &workflow,
        caller.as_ref().map(|Extension(caller)| caller),
    )?))
}
async fn start<E: Env>(
    State(host): State<Host<E>>,
    Extension(Named(service)): Extension<Named>,
    Path(workflow): Path<String>,
    caller: Option<Extension<Caller>>,
    Json(request): Json<Start>,
) -> Result<(StatusCode, Json<Record>), Denied> {
    let caller = caller.map(|Extension(caller)| caller).unwrap_or(Caller {
        id: String::new(),
        key: None,
        app: None,
        user: None,
        client: None,
        agent: None,
    });
    Ok((
        StatusCode::ACCEPTED,
        Json(
            host.start_local_workflow(&service, &workflow, request, caller)
                .await?,
        ),
    ))
}
async fn read<E: Env>(
    State(host): State<Host<E>>,
    Extension(Named(service)): Extension<Named>,
    Path((workflow, run)): Path<(String, String)>,
) -> Result<Json<Record>, Denied> {
    Ok(Json(host.workflow_record(&service, &workflow, &run).await?))
}
async fn continue_run<E: Env>(
    State(host): State<Host<E>>,
    Extension(Named(service)): Extension<Named>,
    Path((workflow, run)): Path<(String, String)>,
) -> Result<(StatusCode, Json<Record>), Denied> {
    host.workflow_record(&service, &workflow, &run).await?;
    let claim = host.claim_workflow(&run, Intent::Continue).await?;
    host.launch_workflow(claim, None).await?;
    Ok((
        StatusCode::ACCEPTED,
        Json(host.workflow_record(&service, &workflow, &run).await?),
    ))
}
async fn cancel<E: Env>(
    State(host): State<Host<E>>,
    Extension(Named(service)): Extension<Named>,
    Path((workflow, run)): Path<(String, String)>,
) -> Result<Json<Record>, Denied> {
    host.workflow_record(&service, &workflow, &run).await?;
    host.cancel_workflow(&run).await?;
    Ok(Json(host.workflow_record(&service, &workflow, &run).await?))
}

async fn no_store(request: Request, next: middleware::Next) -> Response {
    let mut response = next.run(request).await;
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        axum::http::HeaderValue::from_static("private, no-store"),
    );
    response
}
