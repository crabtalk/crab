//! What `#[tool]` expands a function into: the function, its record in the
//! `.berm.abi` section, and the export the host calls.

use berm_abi::{ABI_SECTION, ABI_VERSION, TOOL_PREFIX};
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{
    FnArg, GenericArgument, ItemFn, Pat, PathArguments, ReturnType, Type, ext::IdentExt,
    meta::ParseNestedMeta,
};

/// JSON Schema for a tool that takes no parameters.
const DEFAULT_PARAMETERS: &str = r#"{"type":"object"}"#;

/// What `#[tool(...)]` says of the function.
#[derive(Default)]
pub(crate) struct Options {
    /// `query`: the tool may not write, emit or arm a wake.
    query: bool,
    /// `cron`.
    cron: bool,
    /// `stripe`.
    stripe: bool,
    /// `webhook`.
    webhook: bool,
}

impl Options {
    /// Read one option of `#[tool(...)]`.
    pub(crate) fn parse(&mut self, meta: ParseNestedMeta) -> syn::Result<()> {
        let flag = match &meta.path {
            path if path.is_ident("query") => &mut self.query,
            path if path.is_ident("cron") => &mut self.cron,
            path if path.is_ident("stripe") => &mut self.stripe,
            path if path.is_ident("webhook") => &mut self.webhook,
            _ => {
                return Err(
                    meta.error("unknown option (expected `query`, `cron`, `stripe` or `webhook`)")
                );
            }
        };
        *flag = true;
        Ok(())
    }
}

/// One tool, as its record will describe it.
struct Tool {
    ident: syn::Ident,
    name: String,
    description: String,
    params: Vec<Param>,
    /// The `T` of its `Result<T>`; `None` for `()`, which replies nothing.
    reply: Option<Type>,
    /// From `#[deprecated]`.
    deprecated: bool,
    options: Options,
}

/// One parameter: a property of the tool's arguments.
struct Param {
    ident: syn::Ident,
    ty: Type,
    description: String,
}

/// The tool `#[tool]` with `options` makes of `function`.
pub(crate) fn expand(options: Options, mut function: ItemFn) -> syn::Result<TokenStream> {
    let tool = Tool::read(options, &mut function)?;
    let record = tool.record()?;
    let export = tool.export();
    Ok(quote! {
        #function

        #record

        #export
    })
}

impl Tool {
    /// Read `function` as a tool, stripping the doc comments off its
    /// parameters, which the compiler does not take.
    fn read(options: Options, function: &mut ItemFn) -> syn::Result<Self> {
        let description = crate::docs(&function.attrs);
        let deprecated = function
            .attrs
            .iter()
            .any(|a| a.path().is_ident("deprecated"));

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
            let description = crate::docs(&typed.attrs);
            typed.attrs.retain(|a| !a.path().is_ident("doc"));
            params.push(Param {
                ident: pattern.ident.clone(),
                ty: (*typed.ty).clone(),
                description,
            });
        }

        let ident = &function.sig.ident;
        if [options.cron, options.stripe, options.webhook]
            .into_iter()
            .filter(|set| *set)
            .count()
            > 1
        {
            return Err(syn::Error::new_spanned(
                ident,
                "a tool is at most one of `cron`, `stripe` and `webhook`",
            ));
        }
        if options.cron && !params.is_empty() {
            return Err(syn::Error::new_spanned(
                &function.sig.inputs,
                "a cron tool is called with no arguments",
            ));
        }

        Ok(Tool {
            ident: ident.clone(),
            name: ident.unraw().to_string(),
            description,
            params,
            reply: reply(&function.sig.output)?,
            deprecated,
            options,
        })
    }

    /// Its record in the `.berm.abi` section, naming the package it is
    /// compiled in.
    fn record(&self) -> syn::Result<TokenStream> {
        let Ok(krate) = std::env::var("CARGO_PKG_NAME") else {
            return Err(syn::Error::new_spanned(
                &self.ident,
                "#[tool] needs CARGO_PKG_NAME",
            ));
        };
        let mut out = Pieces::default();
        out.text(&format!(
            r#"{{"tool":{{"name":{},"description":{},"parameters":"#,
            serde_json::Value::from(self.name.as_str()),
            serde_json::Value::from(self.description.as_str()),
        ));
        match self.params.is_empty() {
            true => out.text(DEFAULT_PARAMETERS),
            false => {
                let fields = self.params.iter().map(|param| {
                    let name = param.ident.unraw().to_string();
                    let description = match param.description.is_empty() {
                        true => String::new(),
                        false => serde_json::Value::from(param.description.as_str()).to_string(),
                    };
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
        if let Some(reply) = &self.reply {
            out.text(r#","returns":"#);
            out.shape(quote!(<#reply as ::berm_lang::Schema>::SHAPE));
        }
        let options = &self.options;
        for (set, key) in [
            (options.query, "query"),
            (self.deprecated, "deprecated"),
            (options.cron, "cron"),
            (options.stripe, "stripe"),
            (options.webhook, "webhook"),
        ] {
            if set {
                out.text(&format!(r#","{key}":true"#));
            }
        }
        out.text(&format!(
            r#"}},"abi_version":{ABI_VERSION},"crate":{}}}"#,
            serde_json::Value::from(krate)
        ));
        out.flush();
        let pieces = out.pieces;
        Ok(quote! {
            // This tool's record, in a custom section the host reads without
            // compiling or running the image.
            const _: () = {
                const PIECES: &[::berm_lang::schema::Piece] = &[#(#pieces),*];
                #[used]
                #[cfg_attr(target_arch = "wasm32", unsafe(link_section = #ABI_SECTION))]
                static RECORD: [u8; ::berm_lang::schema::len(PIECES)] =
                    ::berm_lang::schema::render(PIECES);
            };
        })
    }

    /// The export the host calls: it reads the arguments, calls the tool and
    /// writes its reply.
    fn export(&self) -> TokenStream {
        let symbol = format_ident!("{TOOL_PREFIX}{}", self.name);
        let doc = format!("Tool `{}`. {}", self.name, self.description);
        let ident = &self.ident;
        let idents: Vec<_> = self.params.iter().map(|param| &param.ident).collect();
        let types = self.params.iter().map(|param| &param.ty);
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
        let write = match self.reply {
            Some(_) => quote!(::berm_lang::dispatch::reply(&#call)),
            None => quote! {
                #call;
                Ok(::berm_lang::__private::Vec::new())
            },
        };
        quote! {
            #[doc = #doc]
            #[allow(deprecated)]
            #[cfg_attr(target_arch = "wasm32", unsafe(no_mangle))]
            pub extern "C" fn #symbol() {
                ::berm_lang::dispatch::run(
                    |args: &[u8]| -> ::berm_lang::Result<::berm_lang::__private::Vec<u8>> {
                        #read
                        #write
                    },
                )
            }
        }
    }
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

/// A record as JSON text, with each schema left to the type that owns it.
#[derive(Default)]
struct Pieces {
    pieces: Vec<TokenStream>,
    text: String,
}

impl Pieces {
    fn text(&mut self, text: &str) {
        self.text.push_str(text);
    }

    fn shape(&mut self, shape: TokenStream) {
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
