use heck::{ToSnakeCase, ToUpperCamelCase};
use proc_macro2::{Span, TokenStream};
use quote::quote;
use serde_json::Value;
use std::collections::BTreeSet;
use syn::Ident;

#[derive(Default)]
pub(crate) struct Types {
    pub definitions: Vec<TokenStream>,
    names: BTreeSet<String>,
    pub optional: bool,
}

impl Types {
    pub fn ty(&mut self, name: &str, schema: &Value) -> syn::Result<TokenStream> {
        let value = quote!(::berm_lang::serde_json::Value);
        if schema.get("$id").is_some()
            || schema.get("$ref").is_some()
            || schema.get("anyOf").is_some()
            || schema.get("oneOf").is_some()
            || schema.get("allOf").is_some()
            || schema["type"].is_array()
        {
            return Ok(value);
        }
        Ok(match schema["type"].as_str() {
            Some("string") => quote!(::alloc::string::String),
            Some("integer") => quote!(i64),
            Some("number") => quote!(f64),
            Some("boolean") => quote!(bool),
            Some("array") => {
                let ty = self.ty(&format!("{name}Item"), &schema["items"])?;
                quote!(::alloc::vec::Vec<#ty>)
            }
            Some("object")
                if schema["properties"].is_object() && schema["additionalProperties"] == false =>
            {
                self.object(name, schema)?
            }
            Some("object") if schema.get("properties").is_none() => {
                let ty = self.ty(&format!("{name}Value"), &schema["additionalProperties"])?;
                quote!(::alloc::collections::BTreeMap<::alloc::string::String, #ty>)
            }
            _ => value,
        })
    }

    pub fn object(&mut self, name: &str, schema: &Value) -> syn::Result<TokenStream> {
        if !self.names.insert(name.to_owned()) {
            return Err(syn::Error::new(
                Span::call_site(),
                format!("generated type name collision: {name}"),
            ));
        }
        let ident = syn::parse_str::<Ident>(name)?;
        let mut fields = Vec::new();
        let mut names = BTreeSet::new();
        if let Some(properties) = schema["properties"].as_object() {
            for (wire_name, property) in properties {
                let field = identifier(wire_name)?;
                if !names.insert(field.to_string()) {
                    return Err(syn::Error::new(
                        field.span(),
                        "generated field name collision",
                    ));
                }
                let ty = self.ty(
                    &format!("{name}{}", wire_name.to_upper_camel_case()),
                    property,
                )?;
                let required = schema["required"]
                    .as_array()
                    .is_some_and(|required| required.iter().any(|v| v == wire_name));
                let doc = property["description"].as_str().unwrap_or("");
                fields.push(if required {
                    quote! { #[doc = #doc] #[serde(rename = #wire_name)] pub #field: #ty, }
                } else {
                    self.optional = true;
                    quote! {
                        #[doc = #doc]
                        #[serde(rename = #wire_name, default, deserialize_with = "present", skip_serializing_if = "Option::is_none")]
                        pub #field: ::core::option::Option<#ty>,
                    }
                });
            }
        }
        let json = schema.to_string();
        let default = schema["required"]
            .as_array()
            .is_none_or(|required| required.is_empty())
            .then(|| quote!(#[derive(Default)]));
        self.definitions.push(quote! {
            #default
            #[derive(Debug, Clone, ::berm_lang::serde::Serialize, ::berm_lang::serde::Deserialize)]
            #[serde(crate = "::berm_lang::serde", deny_unknown_fields)]
            pub struct #ident { #(#fields)* }
            impl ::berm_lang::Schema for #ident {
                const SHAPE: ::berm_lang::schema::Shape = ::berm_lang::schema::Shape::Json(#json);
            }
        });
        Ok(quote!(#ident))
    }
}

pub(crate) fn identifier(name: &str) -> syn::Result<Ident> {
    let mut normalized = name.to_snake_case();
    if normalized.starts_with(|c: char| c.is_ascii_digit()) {
        normalized.insert(0, '_');
    }
    syn::parse_str::<Ident>(&normalized)
        .or_else(|_| syn::parse_str::<Ident>(&format!("r#{normalized}")))
        .map_err(|_| {
            syn::Error::new(
                Span::call_site(),
                format!("cannot use {name:?} as a Rust identifier"),
            )
        })
}
