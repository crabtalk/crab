use crate::host::{Node, api};
use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    routing,
};
use berm_host::{Denied, deny};
use crabtalk_sdk::{
    account::secret::{self, Create, Grant, Resource},
    client::LOCAL,
    services::service::SetSecret,
};
use std::sync::Arc;

pub(crate) fn router() -> Router<Arc<Node>> {
    Router::new()
        .route(
            &format!("{}/{LOCAL}", secret::PATH),
            routing::get(list).post(create),
        )
        .route(
            &format!("{}/{LOCAL}/{{id}}", secret::PATH),
            routing::put(rotate).delete(remove),
        )
        .route(
            &format!("{}/{LOCAL}/{{id}}/grants", secret::PATH),
            routing::put(grant),
        )
        .route(
            &format!("{}/{LOCAL}/{{id}}/grants/{{service}}", secret::PATH),
            routing::delete(revoke),
        )
}
async fn list(State(node): State<Arc<Node>>) -> Result<Json<Vec<Resource>>, Denied> {
    Ok(Json(node.secrets.resources().map_err(api::internal)?))
}
async fn create(
    State(node): State<Arc<Node>>,
    Json(input): Json<Create>,
) -> Result<Json<Resource>, Denied> {
    input
        .check()
        .map_err(|why| deny(StatusCode::BAD_REQUEST, why))?;
    Ok(Json(
        node.secrets
            .create_resource(&input)
            .map_err(api::internal)?,
    ))
}
async fn rotate(
    State(node): State<Arc<Node>>,
    Path(id): Path<String>,
    Json(input): Json<SetSecret>,
) -> Result<StatusCode, Denied> {
    SetSecret::check_value(&input.value).map_err(|why| deny(StatusCode::BAD_REQUEST, why))?;
    changed(
        node.secrets
            .rotate_resource(&id, &input.value)
            .map_err(api::internal)?,
    )
}
async fn remove(
    State(node): State<Arc<Node>>,
    Path(id): Path<String>,
) -> Result<StatusCode, Denied> {
    changed(node.secrets.delete_resource(&id).map_err(api::internal)?)
}
async fn grant(
    State(node): State<Arc<Node>>,
    Path(id): Path<String>,
    Json(input): Json<Grant>,
) -> Result<StatusCode, Denied> {
    let service = node
        .get(&input.service)
        .ok_or_else(|| deny(StatusCode::NOT_FOUND, "No such service"))?;
    input
        .check(&service.manifest.deps)
        .map_err(|why| deny(StatusCode::BAD_REQUEST, why).explained(secret::GRANT_REFUSED))?;
    changed(
        node.secrets
            .grant_resource(&id, input)
            .map_err(api::internal)?,
    )
}
async fn revoke(
    State(node): State<Arc<Node>>,
    Path((id, service)): Path<(String, String)>,
) -> Result<StatusCode, Denied> {
    changed(
        node.secrets
            .revoke_resource(&id, &service)
            .map_err(api::internal)?,
    )
}
fn changed(found: bool) -> Result<StatusCode, Denied> {
    if found {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(deny(
            StatusCode::NOT_FOUND,
            "No such secret resource or grant",
        ))
    }
}
