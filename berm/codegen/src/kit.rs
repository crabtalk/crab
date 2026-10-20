//! What `kit!` expands into: the kit's record, for the manifest of each
//! program that names the crate in `uses`.

use crate::{config::Kit, utils};
use proc_macro2::TokenStream;
use quote::quote;

pub(crate) fn expand(kit: &Kit) -> TokenStream {
    let package = |key: &str| utils::escape(&std::env::var(key).unwrap_or_default());
    let record = format!(
        r#"{{"name":"{}","version":"{}","deps":{}{}}}"#,
        package("CARGO_PKG_NAME"),
        package("CARGO_PKG_VERSION"),
        kit.declarations.deps(),
        kit.declarations.database(),
    );
    let rebuild = kit.declarations.rebuild();
    quote! {
        /// This kit's record in the manifest of a program that uses it.
        #[doc(hidden)]
        pub const __BERM_KIT: &str = #record;

        #rebuild
    }
}
