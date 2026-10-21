//! What `#[program]` expands a module of tool functions into.

use crate::{config::Config, utils};
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{
    FnArg, GenericArgument, Item, ItemFn, ItemMod, Pat, PathArguments, ReturnType, Type,
    ext::IdentExt,
};

/// JSON Schema for a tool that takes no parameters.
const DEFAULT_PARAMETERS: &str = r#"{"type":"object"}"#;

/// Prefix on every tool's exported symbol, so a tool called `init` or
/// `describe` cannot collide with the exports the ABI reserves.
const TOOL_PREFIX: &str = "berm_tool_";

/// Prefix on the function in the program's module that reads a tool's
/// arguments, calls it and writes its reply.
const DISPATCH_PREFIX: &str = "__berm_";

/// One exported tool, as the description will see it.
struct Tool {
    ident: syn::Ident,
    name: String,
    description: String,
    params: Vec<Param>,
    /// The `T` of its `Result<T>`; `None` for `()`, which replies nothing.
    reply: Option<Type>,
    /// From `#[query]`: the tool may not write, emit or arm a wake.
    query: bool,
    /// From `#[deprecated]`.
    deprecated: bool,
    /// From `#[public]`: callers from outside and other programs may reach it.
    public: bool,
    /// From `#[anonymous]`: a call from outside with no caller may reach it.
    anonymous: bool,
    /// From `#[callers(services)]` or `#[callers(accounts)]`.
    callers: Option<String>,
    /// From `#[cron]`.
    cron: bool,
    /// From `#[stripe]`.
    stripe: bool,
}

/// One parameter: a property of the tool's arguments.
struct Param {
    ident: syn::Ident,
    ty: Type,
    description: String,
}

/// The program `#[program]` with `config` makes of `module`.
pub(crate) fn expand(config: &Config, mut module: ItemMod) -> TokenStream {
    let tools = match collect(&mut module) {
        Ok(tools) => tools,
        Err(error) => return error.to_compile_error(),
    };

    let pieces = manifest(config, &tools);
    let module_ident = &module.ident;
    let buffer = config.buffer;

    let (_, items) = module.content.as_mut().expect("collect checked the body");
    for tool in &tools {
        items.push(dispatch(tool));
    }
    // In the module, where the types its shapes name resolve.
    items.push(syn::parse_quote! {
        // What this program is, as a section rather than an export: the host
        // reads it out of the image without compiling or running anything, so
        // learning what a program claims never means executing it.
        const _: () = {
            const PIECES: &[::berm_lang::schema::Piece] = &[#(#pieces),*];
            #[used]
            #[cfg_attr(target_arch = "wasm32", unsafe(link_section = ".berm.abi"))]
            static ABI: [u8; ::berm_lang::schema::len(PIECES)] =
                ::berm_lang::schema::render(PIECES);
        };
    });

    // One export per tool, so the host resolves it the way it resolves any
    // other symbol.
    let exports = tools.iter().map(|tool| {
        let symbol = format_ident!("{TOOL_PREFIX}{}", tool.name);
        let dispatch = format_ident!("{DISPATCH_PREFIX}{}", tool.name);
        let doc = format!("Tool `{}`. {}", tool.name, tool.description);

        quote! {
            #[doc = #doc]
            #[unsafe(no_mangle)]
            pub extern "C" fn #symbol() {
                let arguments = unsafe { &mut *::core::ptr::addr_of_mut!(_BERM_ARGS) };
                let length = ::berm_lang::abi::read_args(arguments);
                if length > _BERM_BUFFER {
                    return ::berm_lang::abi::fail(b"arguments exceeded the input buffer");
                }
                match #module_ident::#dispatch(&arguments[..length]) {
                    Ok(reply) if reply.len() > _BERM_BUFFER => {
                        ::berm_lang::abi::fail(b"result exceeded the output buffer")
                    }
                    Ok(reply) => ::berm_lang::abi::done(&reply),
                    Err(error) => ::berm_lang::abi::fail(error.message.as_bytes()),
                }
            }
        }
    });

    // The macro read the README itself, which cargo cannot see. This makes
    // the dependency visible so editing it rebuilds the program.
    let readme_rebuild = config.readme_file.as_ref().map(|path| {
        quote! {
            const _BERM_README: &str =
                include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/", #path));
        }
    });

    // Each migration was read by the macro too.
    let database_rebuild = config.declarations.rebuild();

    quote! {
        #module

        #readme_rebuild

        #database_rebuild

        const _BERM_BUFFER: usize = #buffer;
        static mut _BERM_ARGS: [u8; _BERM_BUFFER] = [0; _BERM_BUFFER];

        /// A program is a binary, and off a guest target a binary needs a
        /// `main` — which is what lets `cargo test` build one natively.
        #[cfg(not(target_arch = "wasm32"))]
        fn main() {}

        #(#exports)*
    }
}

/// Pull the tools out of the module, stripping the attributes that only exist
/// to be read here.
fn collect(module: &mut ItemMod) -> syn::Result<Vec<Tool>> {
    let Some((_, items)) = module.content.as_mut() else {
        return Err(syn::Error::new_spanned(
            &module.ident,
            "#[program] needs a module with a body, not a `mod foo;` declaration",
        ));
    };

    let mut tools = Vec::new();
    for item in items.iter_mut() {
        let Item::Fn(function) = item else { continue };
        if !matches!(function.vis, syn::Visibility::Public(_)) {
            continue;
        }
        tools.push(tool(function)?);
    }

    if tools.is_empty() {
        return Err(syn::Error::new_spanned(
            &module.ident,
            "a program needs at least one `pub fn` to expose as a tool",
        ));
    }
    if let [_, second, ..] = tools
        .iter()
        .filter(|tool| tool.stripe)
        .collect::<Vec<_>>()
        .as_slice()
    {
        return Err(syn::Error::new_spanned(
            &second.ident,
            "a program has at most one `#[stripe]` tool",
        ));
    }
    Ok(tools)
}

fn tool(function: &mut ItemFn) -> syn::Result<Tool> {
    let description = utils::docs(&function.attrs);
    let mut query = false;
    let mut public = false;
    let mut anonymous = false;
    let mut callers = None;
    let mut cron = false;
    let mut stripe = false;
    let deprecated = function
        .attrs
        .iter()
        .any(|a| a.path().is_ident("deprecated"));

    for attribute in &function.attrs {
        if attribute.path().is_ident("query") {
            attribute.meta.require_path_only()?;
            query = true;
        }
        if attribute.path().is_ident("public") {
            attribute.meta.require_path_only()?;
            public = true;
        }
        if attribute.path().is_ident("anonymous") {
            attribute.meta.require_path_only()?;
            anonymous = true;
        }
        if attribute.path().is_ident("callers") {
            let kind: syn::Ident = attribute.parse_args()?;
            if kind != "services" && kind != "accounts" {
                return Err(syn::Error::new(
                    kind.span(),
                    "expected `services` or `accounts`",
                ));
            }
            callers = Some(kind.to_string());
        }
        if attribute.path().is_ident("cron") {
            attribute.meta.require_path_only()?;
            cron = true;
        }
        if attribute.path().is_ident("stripe") {
            attribute.meta.require_path_only()?;
            stripe = true;
        }
    }

    // These exist for this macro to read; leaving them would reach the
    // compiler as unknown attributes.
    function.attrs.retain(|a| {
        ![
            "query",
            "public",
            "anonymous",
            "callers",
            "deprecated",
            "cron",
            "stripe",
        ]
        .iter()
        .any(|name| a.path().is_ident(name))
    });

    if description.is_empty() {
        return Err(syn::Error::new_spanned(
            &function.sig.ident,
            "a tool needs a doc comment — it is what the model reads to decide whether to call it",
        ));
    }

    let mut params = Vec::new();
    for input in function.sig.inputs.iter_mut() {
        let FnArg::Typed(typed) = input else {
            return Err(syn::Error::new_spanned(input, "a tool is a free function"));
        };
        let Pat::Ident(pattern) = &*typed.pat else {
            return Err(syn::Error::new_spanned(
                &typed.pat,
                "a tool's parameter is a plain name — it is the argument's key",
            ));
        };
        let description = utils::docs(&typed.attrs);
        typed.attrs.retain(|a| !a.path().is_ident("doc"));
        params.push(Param {
            ident: pattern.ident.clone(),
            ty: (*typed.ty).clone(),
            description,
        });
    }

    if cron && public {
        return Err(syn::Error::new_spanned(
            &function.sig.ident,
            "a cron tool is called by its host, and cannot be `#[public]`",
        ));
    }
    if stripe && public {
        return Err(syn::Error::new_spanned(
            &function.sig.ident,
            "a stripe tool is called by its host, and cannot be `#[public]`",
        ));
    }
    if stripe && cron {
        return Err(syn::Error::new_spanned(
            &function.sig.ident,
            "a tool is `#[cron]` or `#[stripe]`, not both",
        ));
    }
    if anonymous && !public {
        return Err(syn::Error::new_spanned(
            &function.sig.ident,
            "an `#[anonymous]` tool is called from outside, and must be `#[public]`",
        ));
    }
    if anonymous && callers.as_deref() == Some("services") {
        return Err(syn::Error::new_spanned(
            &function.sig.ident,
            "an `#[anonymous]` tool is called from outside, and cannot be `#[callers(services)]`",
        ));
    }
    if cron && !params.is_empty() {
        return Err(syn::Error::new_spanned(
            &function.sig.inputs,
            "a cron tool is called with no arguments",
        ));
    }

    Ok(Tool {
        ident: function.sig.ident.clone(),
        name: function.sig.ident.to_string(),
        description,
        params,
        reply: reply(&function.sig.output)?,
        query,
        deprecated,
        public,
        anonymous,
        callers,
        cron,
        stripe,
    })
}

/// The `T` of a tool's `Result<T>`, or `None` for `()`.
fn reply(output: &ReturnType) -> syn::Result<Option<Type>> {
    let wrong = || {
        syn::Error::new_spanned(
            output,
            "a tool returns `berm_lang::Result<T>`, `T` being its reply",
        )
    };
    let ReturnType::Type(_, ty) = output else {
        return Err(wrong());
    };
    let Type::Path(path) = &**ty else {
        return Err(wrong());
    };
    let Some(segment) = path.path.segments.last() else {
        return Err(wrong());
    };
    let PathArguments::AngleBracketed(args) = &segment.arguments else {
        return Err(wrong());
    };
    if segment.ident != "Result" {
        return Err(wrong());
    }
    let Some(GenericArgument::Type(reply)) = args.args.first() else {
        return Err(wrong());
    };
    Ok(match reply {
        Type::Tuple(unit) if unit.elems.is_empty() => None,
        reply => Some(reply.clone()),
    })
}

/// The function in the program's module that reads `tool`'s arguments, calls
/// it and writes its reply.
fn dispatch(tool: &Tool) -> Item {
    let name = format_ident!("{DISPATCH_PREFIX}{}", tool.name);
    let ident = &tool.ident;
    let idents: Vec<_> = tool.params.iter().map(|param| &param.ident).collect();
    let types = tool.params.iter().map(|param| &param.ty);
    let read = match idents.is_empty() {
        true => quote!(let _ = args;),
        false => quote! {
            #[derive(::berm_lang::serde::Deserialize)]
            #[serde(crate = "::berm_lang::serde")]
            struct __BermArgs {
                #(#idents: #types,)*
            }
            let __BermArgs { #(#idents),* } = ::berm_lang::dispatch::args(args)?;
        },
    };
    let call = quote!(#ident(#(#idents),*)?);
    let write = match tool.reply {
        Some(_) => quote!(::berm_lang::dispatch::reply(&#call)),
        None => quote! {
            #call;
            Ok(::berm_lang::__private::Vec::new())
        },
    };
    syn::parse_quote! {
        #[doc(hidden)]
        pub(crate) fn #name(
            args: &[u8],
        ) -> ::berm_lang::Result<::berm_lang::__private::Vec<u8>> {
            #read
            #write
        }
    }
}

/// A manifest as JSON text, with each tool's schemas left to the types that
/// own them.
struct Pieces {
    pieces: Vec<proc_macro2::TokenStream>,
    text: String,
}

impl Pieces {
    fn text(&mut self, text: &str) {
        self.text.push_str(text);
    }

    fn shape(&mut self, shape: proc_macro2::TokenStream) {
        self.flush();
        self.pieces
            .push(quote!(::berm_lang::schema::Piece::Shape(&#shape)));
    }

    fn flush(&mut self) {
        if !self.text.is_empty() {
            let text = std::mem::take(&mut self.text);
            self.pieces
                .push(quote!(::berm_lang::schema::Piece::Text(#text)));
        }
    }
}

/// The program's record in the manifest section.
fn manifest(config: &Config, tools: &[Tool]) -> Vec<proc_macro2::TokenStream> {
    let mut out = Pieces {
        pieces: Vec::new(),
        text: String::new(),
    };
    let package = |key: &str| utils::escape(&std::env::var(key).unwrap_or_default());
    out.text(&format!(
        r#"{{"abi_version":4,"version":"{}","description":"{}","homepage":"{}","repository":"{}","tools":["#,
        package("CARGO_PKG_VERSION"),
        package("CARGO_PKG_DESCRIPTION"),
        package("CARGO_PKG_HOMEPAGE"),
        package("CARGO_PKG_REPOSITORY"),
    ));
    for (at, tool) in tools.iter().enumerate() {
        if at > 0 {
            out.text(",");
        }
        out.text(&format!(
            r#"{{"name":"{}","description":"{}","parameters":"#,
            utils::escape(&tool.name),
            utils::escape(&tool.description),
        ));
        match tool.params.is_empty() {
            true => out.text(DEFAULT_PARAMETERS),
            false => {
                let fields = tool.params.iter().map(|param| {
                    let name = param.ident.unraw().to_string();
                    let description = utils::escape(&param.description);
                    let ty = &param.ty;
                    quote! {
                        ::berm_lang::schema::Field {
                            name: #name,
                            description: #description,
                            shape: &<#ty as ::berm_lang::Schema>::SHAPE,
                        }
                    }
                });
                out.shape(quote! {
                    ::berm_lang::schema::Shape::Object { title: "", fields: &[#(#fields),*] }
                });
            }
        }
        if let Some(reply) = &tool.reply {
            out.text(r#","returns":"#);
            out.shape(quote!(<#reply as ::berm_lang::Schema>::SHAPE));
        }
        if tool.query {
            out.text(r#","query":true"#);
        }
        if tool.deprecated {
            out.text(r#","deprecated":true"#);
        }
        if tool.public {
            out.text(r#","public":true"#);
        }
        if tool.anonymous {
            out.text(r#","anonymous":true"#);
        }
        if let Some(kind) = &tool.callers {
            out.text(&format!(r#","callers":"{kind}""#));
        }
        if tool.cron {
            out.text(r#","cron":true"#);
        }
        if tool.stripe {
            out.text(r#","stripe":true"#);
        }
        out.text("}");
    }

    out.text(&format!(
        r#"],"readme":"{}","deps":{}{}"#,
        utils::escape(&config.readme),
        config.declarations.deps(),
        config.declarations.database(),
    ));
    // Each kit's record is a const of the kit's crate, rendered in its place.
    if !config.uses.is_empty() {
        out.text(r#","kits":["#);
        for (at, kit) in config.uses.iter().enumerate() {
            if at > 0 {
                out.text(",");
            }
            out.flush();
            out.pieces
                .push(quote!(::berm_lang::schema::Piece::Text(#kit::__BERM_KIT)));
        }
        out.text("]");
    }
    out.text("}");
    out.flush();
    out.pieces
}
