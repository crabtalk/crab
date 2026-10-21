//! The dashboard's static build, served at `/` beside the control API.
//!
//! The build is hash-routed: every page is `index.html`, and a path names only
//! an asset under `_app/`.

use axum::{
    http::{StatusCode, Uri, header},
    response::{IntoResponse, Response},
};
use rust_embed::Embed;

/// Read from disk on each request in debug builds, embedded in release ones.
#[derive(Embed)]
#[folder = "../../dashboard/build"]
struct Build;

/// Answer a `GET` no other route takes with the asset it names, `/` with
/// `index.html`.
pub async fn serve(uri: Uri) -> Response {
    let path = match uri.path().trim_start_matches('/') {
        "" => "index.html",
        path => path,
    };
    match Build::get(path) {
        Some(file) => (
            [(header::CONTENT_TYPE, file.metadata.mimetype().to_owned())],
            file.data,
        )
            .into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}
