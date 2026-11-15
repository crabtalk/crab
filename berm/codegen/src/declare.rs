//! `declare_service!` — typed calls and types from another service's IDL.
//!
//! The IDL is the JSON a host serves for a service: a deployed service as the control API
//! reports it. It is read from `idls/{name}.json` in the crate's directory or
//! the nearest ancestor that has one.

use berm_api::schema;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use serde_json::Value;
use std::{collections::BTreeMap, path::PathBuf};
use syn::Ident;

pub fn expand(name: &Ident) -> syn::Result<TokenStream> {
    let path = find(name)?;
    let text = std::fs::read_to_string(&path)
        .map_err(|error| syn::Error::new(name.span(), format!("{}: {error}", path.display())))?;
    let idl: Value = serde_json::from_str(&text)
        .map_err(|error| syn::Error::new(name.span(), format!("{}: {error}", path.display())))?;
    let fail =
        |message: &str| syn::Error::new(name.span(), format!("{}: {message}", path.display()));

    let service_name = idl["name"].as_str().ok_or_else(|| fail("no `name`"))?;
    let service = match idl["owner"].as_str().filter(|owner| !owner.is_empty()) {
        Some(owner) => format!("{owner}/{service_name}"),
        None => service_name.to_owned(),
    };
    let tools = idl["tools"].as_array().ok_or_else(|| fail("no `tools`"))?;

    // Every titled object anywhere in the IDL is one struct, by its title.
    let mut structs = BTreeMap::new();
    for tool in tools {
        collect(&tool["parameters"], &mut structs);
        collect(&tool["returns"], &mut structs);
    }

    let mut descriptor_outputs = BTreeMap::new();
    for tool in tools {
        let mut output = tool["returns"].clone();
        if schema::variants(&output).is_some() && output["title"].is_null() {
            let name = tool["name"].as_str().unwrap_or_default();
            let title = format!("{}Output", camel(name));
            if structs.contains_key(&title) {
                return Err(fail(&format!(
                    "generated workflow response {title} conflicts with an IDL type"
                )));
            }
            output["title"] = Value::from(title);
            collect(&output, &mut structs);
        }
        descriptor_outputs.insert(tool["name"].as_str().unwrap_or_default(), output);
    }

    let types = structs.iter().map(|(title, schema)| {
        let ident = format_ident!("{title}");
        let schema_impl = schema_impl(&ident, schema);
        let branch_impl = branch(&ident, schema);
        let doc = schema["description"]
            .as_str()
            .map(|text| quote!(#[doc = #text]));
        if let Some(values) = schema::values(schema) {
            let variants = values.into_iter().map(|value| {
                let variant = format_ident!("{}", camel(value));
                quote! {
                    #[serde(rename = #value)]
                    #variant,
                }
            });
            return quote! {
                #schema_impl
                #branch_impl
                #doc
                #[derive(
                    Debug,
                    Clone,
                    Copy,
                    PartialEq,
                    Eq,
                    ::berm_lang::serde::Serialize,
                    ::berm_lang::serde::Deserialize,
                )]
                #[serde(crate = "::berm_lang::serde")]
                pub enum #ident {
                    #(#variants)*
                }
            };
        }
        if let Some(alternatives) = schema::variants(schema) {
            let variants = alternatives.into_iter().map(|(name, payload)| {
                let variant = format_ident!("{}", camel(name));
                let Some(payload) = payload else {
                    return quote!(#[serde(rename = #name)] #variant,);
                };
                let fields = if payload["type"] == "object" && payload["title"].is_null() && payload["properties"].is_object() {
                    let fields = fields(payload, false, &quote!());
                    quote!({ #(#fields)* })
                } else {
                    let ty = rust_type(payload, &quote!());
                    quote!((#ty))
                };
                quote!(#[serde(rename = #name)] #variant #fields,)
            });
            return quote! {
                #schema_impl
                #branch_impl
                #doc
                #[derive(Debug, Clone, ::berm_lang::serde::Serialize, ::berm_lang::serde::Deserialize)]
                #[serde(crate = "::berm_lang::serde")]
                pub enum #ident { #(#variants)* }
            };
        }
        let fields = fields(schema, true, &quote!());
        quote! {
            #schema_impl
            #doc
            #[derive(
                Debug,
                Clone,
                ::berm_lang::serde::Serialize,
                ::berm_lang::serde::Deserialize,
            )]
            #[serde(crate = "::berm_lang::serde")]
            pub struct #ident {
                #(#fields)*
            }
        }
    });

    let descriptors = tools.iter().map(|tool| {
        let tool_name = tool["name"].as_str().unwrap_or_default();
        let ident = format_ident!("{}Call", camel(tool_name));
        let parameters = &tool["parameters"];
        let fields = fields(parameters, true, &quote!(types::));
        let schema_impl = schema_impl(&ident, parameters);
        let output_schema = &descriptor_outputs[tool_name];
        let output = if output_schema.is_object() {
            rust_type(output_schema, &quote!(types::))
        } else {
            quote!(())
        };
        let cases = if schema::variants(output_schema).is_some() && output_schema["title"].is_string() {
            quote!(const CASES: &'static [::berm_lang::schema::Variant] = <#output as ::berm_lang::workflow::Branch>::CASES;)
        } else { quote!() };
        let window = match tool["idempotency_ms"].as_u64() {
            Some(ms) => quote!(Some(#ms)),
            None => quote!(None),
        };
        quote! {
            #[derive(::berm_lang::serde::Serialize, ::berm_lang::serde::Deserialize)]
            #[serde(crate = "::berm_lang::serde")]
            pub struct #ident { #(#fields)* }
            #schema_impl
            impl ::berm_lang::workflow::Call for #ident {
                const TARGET: ::berm_lang::workflow::Target = ::berm_lang::workflow::Target::Service {
                    service: #service, tool: #tool_name, idempotency_ms: #window,
                };
                type Output = #output;
                #cases
            }
        }
    });

    let calls = tools.iter().map(|tool| {
        let tool_name = tool["name"].as_str().unwrap_or_default();
        let ident = field_ident(tool_name);
        let doc = tool["description"].as_str().unwrap_or_default();
        let deprecated = deprecation(tool);
        let types = quote!(super::types::);
        let parameters = &tool["parameters"];
        let required = required(parameters);
        let properties: Vec<_> = parameters["properties"]
            .as_object()
            .into_iter()
            .flatten()
            .map(|(name, property)| {
                let ident = field_ident(name);
                let ty = rust_type(property, &types);
                match required.contains(&name.as_str()) {
                    true => (
                        quote!(#ident: #ty),
                        quote! {
                            #[serde(rename = #name)]
                            #ident: #ty,
                        },
                    ),
                    false => (
                        quote!(#ident: ::core::option::Option<#ty>),
                        quote! {
                            #[serde(
                                rename = #name,
                                skip_serializing_if = "::core::option::Option::is_none"
                            )]
                            #ident: ::core::option::Option<#ty>,
                        },
                    ),
                }
            })
            .collect();
        let params = properties.iter().map(|(param, _)| param);
        let encode = match properties.is_empty() {
            true => quote!(let args = ::berm_lang::__private::String::new();),
            false => {
                let fields = properties.iter().map(|(_, field)| field);
                let idents = parameters["properties"]
                    .as_object()
                    .into_iter()
                    .flatten()
                    .map(|(name, _)| field_ident(name));
                quote! {
                    #[derive(::berm_lang::serde::Serialize)]
                    #[serde(crate = "::berm_lang::serde")]
                    struct Args {
                        #(#fields)*
                    }
                    let args = ::berm_lang::json::to(&Args { #(#idents),* })?;
                }
            }
        };
        let (returns, decode) = match tool["returns"].is_object() {
            true => (
                rust_type(&tool["returns"], &types),
                quote!(::berm_lang::json::from(&result)),
            ),
            false => (
                quote!(()),
                quote! {
                    let _ = result;
                    ::core::result::Result::Ok(())
                },
            ),
        };
        quote! {
            #[doc = #doc]
            #deprecated
            pub fn #ident(#(#params),*) -> ::core::result::Result<#returns, ::berm_lang::CallError> {
                #encode
                let result = ::berm_lang::call(super::NAME, #tool_name, &args)?;
                #decode
            }
        }
    });

    let module = name;
    let source = path.display().to_string();
    Ok(quote! {
        pub mod #module {
            // Rebuilds this crate when the IDL changes.
            const _: &str = include_str!(#source);

            /// What the service is deployed as, which every call names.
            pub const NAME: &str = #service;

            /// Its argument and result structs.
            #[allow(deprecated)]
            pub mod types {
                #(#types)*
            }

            #(#descriptors)*

            /// Its tools, each a `berm_lang::call` with typed arguments and
            /// result.
            pub mod calls {
                #(#calls)*
            }
        }
    })
}

/// `#[deprecated]` for a tool the IDL marks deprecated.
fn deprecation(spec: &Value) -> TokenStream {
    match spec["deprecated"].as_bool() == Some(true) {
        true => quote!(#[deprecated]),
        false => quote!(),
    }
}

/// `idls/{name}.json` in the crate's directory or the nearest ancestor.
fn find(name: &Ident) -> syn::Result<PathBuf> {
    let root = std::env::var("CARGO_MANIFEST_DIR")
        .map_err(|_| syn::Error::new(name.span(), "declare_service! needs CARGO_MANIFEST_DIR"))?;
    let file = format!("{name}.json");
    PathBuf::from(&root)
        .ancestors()
        .map(|dir| dir.join("idls").join(&file))
        .find(|path| path.is_file())
        .ok_or_else(|| {
            syn::Error::new(
                name.span(),
                format!("no idls/{file} in {root} or above it; save the IDL of the service deployed as `{name}` there"),
            )
        })
}

/// Every titled object in `schema`, by title.
fn collect(schema: &Value, structs: &mut BTreeMap<String, Value>) {
    if let Some(title) = schema["title"].as_str() {
        structs.insert(title.to_owned(), schema.clone());
    }
    for property in schema["properties"].as_object().into_iter().flatten() {
        collect(property.1, structs);
    }
    for union in ["anyOf", "oneOf"] {
        for variant in schema[union].as_array().into_iter().flatten() {
            collect(variant, structs);
        }
    }
    collect_inner(&schema["items"], structs);
    collect_inner(&schema["additionalProperties"], structs);
}

fn fields(schema: &Value, public: bool, path: &TokenStream) -> Vec<TokenStream> {
    let required = required(schema);
    let visibility = public.then(|| quote!(pub));
    schema["properties"].as_object().into_iter().flatten().map(|(field, property)| {
        let ident = field_ident(field);
        let ty = rust_type(property, path);
        let doc = property["description"].as_str().map(|text| quote!(#[doc = #text]));
        if required.contains(&field.as_str()) {
            quote!(#doc #[serde(rename = #field)] #visibility #ident: #ty,)
        } else {
            quote!(#doc #[serde(rename = #field, default, skip_serializing_if = "::core::option::Option::is_none")] #visibility #ident: ::core::option::Option<#ty>,)
        }
    }).collect()
}

fn collect_inner(schema: &Value, structs: &mut BTreeMap<String, Value>) {
    if schema.is_object() {
        collect(schema, structs);
    }
}

/// The Rust type a schema describes. `path` prefixes a struct's name.
fn rust_type(schema: &Value, path: &TokenStream) -> TokenStream {
    if let Some(inner) = schema::nullable(schema) {
        let inner = rust_type(inner, path);
        return quote!(::core::option::Option<#inner>);
    }
    if let Some(title) = schema["title"].as_str() {
        let ident = format_ident!("{title}");
        return quote!(#path #ident);
    }
    match schema["type"].as_str() {
        Some("string") => quote!(::berm_lang::__private::String),
        Some("boolean") => quote!(bool),
        Some("number") => quote!(f64),
        Some("integer") if schema["minimum"].as_u64() == Some(0) => quote!(u64),
        Some("integer") => quote!(i64),
        Some("array") => {
            let items = rust_type(&schema["items"], path);
            quote!(::berm_lang::__private::Vec<#items>)
        }
        Some("object") if schema["additionalProperties"].is_object() => {
            let values = rust_type(&schema["additionalProperties"], path);
            quote!(::berm_lang::__private::BTreeMap<::berm_lang::__private::String, #values>)
        }
        _ => quote!(::berm_lang::serde_json::Value),
    }
}

/// The names an object schema requires.
fn required(schema: &Value) -> Vec<&str> {
    schema["required"]
        .as_array()
        .map(|names| names.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default()
}

/// A `snake_case` enum value as the `CamelCase` variant it was written from.
fn camel(value: &str) -> String {
    value
        .split('_')
        .map(|word| {
            let mut chars = word.chars();
            chars
                .next()
                .map(|first| first.to_ascii_uppercase().to_string() + chars.as_str())
                .unwrap_or_default()
        })
        .collect()
}

/// A Rust identifier for a JSON name, raw when the name is a keyword.
fn field_ident(name: &str) -> Ident {
    syn::parse_str::<Ident>(name)
        .unwrap_or_else(|_| Ident::new_raw(name, proc_macro2::Span::call_site()))
}

fn schema_impl(ident: &Ident, schema: &Value) -> TokenStream {
    let schema = schema.to_string();
    quote! {
        impl ::berm_lang::Schema for #ident {
            const SHAPE: ::berm_lang::schema::Shape = ::berm_lang::schema::Shape::Json(#schema);
        }
    }
}

fn branch(ident: &Ident, schema: &Value) -> TokenStream {
    let Some(variants) = schema::variants(schema) else {
        return quote!();
    };
    let mut payloads = Vec::new();
    let mut cases = Vec::new();
    let mut arms = Vec::new();
    let mut projections = Vec::new();
    for (name, payload) in variants {
        let variant = format_ident!("{}", camel(name));
        let ty = format_ident!("{ident}{variant}Payload");
        let (item, pattern, constructor, case_pattern) = match payload {
            None => (
                quote!(#[derive(::berm_lang::Schema)] pub struct #ty {}),
                quote!(Self::#variant),
                quote!(#ty {}),
                quote!(Self::#variant),
            ),
            Some(payload)
                if payload["type"] == "object"
                    && payload["title"].is_null()
                    && payload["properties"].is_object() =>
            {
                let fields = fields(payload, true, &quote!());
                let idents: Vec<_> = payload["properties"]
                    .as_object()
                    .into_iter()
                    .flatten()
                    .map(|(name, _)| field_ident(name))
                    .collect();
                let shape = schema_impl(&ty, payload);
                let item = quote! {
                    #[derive(::berm_lang::serde::Serialize, ::berm_lang::serde::Deserialize)]
                    #[serde(crate = "::berm_lang::serde")]
                    pub struct #ty { #(#fields)* }
                    #shape
                };
                (
                    item,
                    quote!(Self::#variant { #(#idents),* }),
                    quote!(#ty { #(#idents),* }),
                    quote!(Self::#variant { .. }),
                )
            }
            Some(payload) => {
                let inner = rust_type(payload, &quote!());
                let shape = schema_impl(&ty, payload);
                (
                    quote! {
                        #[derive(::berm_lang::serde::Serialize, ::berm_lang::serde::Deserialize)]
                        #[serde(crate = "::berm_lang::serde", transparent)]
                        pub struct #ty(pub #inner);
                        #shape
                    },
                    quote!(Self::#variant(payload)),
                    quote!(#ty(payload)),
                    quote!(Self::#variant(_)),
                )
            }
        };
        payloads.push(item);
        arms.push(quote!(#case_pattern => #name));
        cases.push(quote!(::berm_lang::schema::Variant { name: #name, description: "", payload: Some(&<#ty as ::berm_lang::Schema>::SHAPE) }));
        projections.push(quote! {
            impl ::berm_lang::workflow::Projection<#ty> for #ident {
                const CASE: &'static str = #name;
                fn project(self) -> ::core::option::Option<#ty> {
                    match self { #pattern => ::core::option::Option::Some(#constructor), _ => ::core::option::Option::None }
                }
            }
        });
    }
    quote! {
        #(#payloads)*
        impl ::berm_lang::workflow::Branch for #ident {
            const CASES: &'static [::berm_lang::schema::Variant] = &[#(#cases),*];
            fn case(&self) -> &'static str { match self { #(#arms),* } }
        }
        #(#projections)*
    }
}
