//! Compile a local OpenAPI 3.0 or 3.1 document into berm service APIs and tools.
use proc_macro::TokenStream;
use syn::parse_macro_input;
mod config;
mod document;
mod schema;
mod v30;

/// Generate `api`, `types` and, unless disabled, `tools` modules.
/// Paths are relative to the consuming crate's manifest directory.
#[proc_macro]
pub fn service(input: TokenStream) -> TokenStream {
    let config = parse_macro_input!(input as config::Config);
    document::Document::read(&config)
        .and_then(|document| document.generate(&config))
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}
