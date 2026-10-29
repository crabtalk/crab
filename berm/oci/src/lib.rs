//! Publishing and fetching services as OCI artifacts.
//!
//! A service is one layer and no tarball, so the layer's digest is sha256 of
//! the module — the same hash a host reports for the service, carrying the registry's `sha256:`
//! prefix. What the service *is* rides in the config blob: the image's
//! manifest as JSON, so a registry can be listed without pulling any image.

mod reference;
mod registry;

pub use reference::Reference;
pub use registry::{Access, Registry};
