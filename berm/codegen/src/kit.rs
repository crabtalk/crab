//! What `kit!` expands into: the kit's record, for the manifest of each
//! program that names the crate in `uses`.

use crate::{config::Declarations, utils};
use proc_macro2::{Span, TokenStream};
use quote::quote;
use std::env;

pub(crate) fn expand(declarations: &Declarations) -> TokenStream {
    let Ok(name) = env::var("CARGO_PKG_NAME") else {
        return syn::Error::new(Span::call_site(), "kit! needs CARGO_PKG_NAME").to_compile_error();
    };
    let record = format!(
        r#"{{"name":"{}","version":"{}","deps":{}{}}}"#,
        utils::escape(&name),
        utils::escape(&env::var("CARGO_PKG_VERSION").unwrap_or_default()),
        declarations.deps(),
        declarations.database(),
    );
    let rebuild = declarations.rebuild();
    quote! {
        /// This kit's record in the manifest of a program that uses it.
        #[doc(hidden)]
        pub const __BERM_KIT: &str = #record;

        #rebuild
    }
}
