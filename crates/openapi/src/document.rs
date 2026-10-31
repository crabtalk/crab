use crate::{
    config::Config,
    schema::{self, Types},
    v30,
};
use heck::ToUpperCamelCase;
use oas3::{
    Spec,
    spec::{Parameter, ParameterIn, ParameterStyle},
};
use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote};
use serde_json::{Value, json};
use std::{cell::Cell, collections::BTreeSet, env, fs, path::PathBuf};

#[derive(Clone, Copy, PartialEq, Eq)]
enum ParameterShape {
    Scalar,
    Array,
    Object,
}

pub(crate) struct Document {
    spec: Spec,
    source: Value,
    path: PathBuf,
    next_schema_id: Cell<usize>,
}

impl Document {
    pub fn read(config: &Config) -> syn::Result<Self> {
        let path = PathBuf::from(env::var("CARGO_MANIFEST_DIR").map_err(Self::error)?)
            .join(config.spec.value());
        let source =
            fs::read_to_string(&path).map_err(|e| syn::Error::new(config.spec.span(), e))?;
        let mut source: Value = serde_json::from_str(&source).map_err(Self::error)?;
        match source["openapi"].as_str() {
            Some(version) if version.starts_with("3.0.") => {
                let _: openapiv3::OpenAPI =
                    serde_json::from_value(source.clone()).map_err(Self::error)?;
                v30::normalize(&mut source)?;
            }
            Some(version) if version.starts_with("3.1.") => {}
            _ => return Err(Self::error("expected OpenAPI 3.0 or 3.1")),
        }
        let spec = serde_json::from_value(source.clone()).map_err(Self::error)?;
        Ok(Self {
            spec,
            source,
            path,
            next_schema_id: Cell::new(0),
        })
    }

    pub fn generate(&self, config: &Config) -> syn::Result<TokenStream> {
        let mut types = Types::default();
        let mut needs_scalar = false;
        let (mut functions, mut tools, mut names) = (Vec::new(), Vec::new(), BTreeSet::new());
        for (name, selector) in &config.operations {
            if !names.insert(name.to_string()) {
                return Err(syn::Error::new(name.span(), "duplicate operation name"));
            }
            let generated = self
                .operation(config, name, &selector.value(), &mut types)
                .map_err(|e| {
                    syn::Error::new(selector.span(), format!("{}: {e}", selector.value()))
                })?;
            functions.push(generated.0);
            tools.push(generated.1);
            needs_scalar |= generated.2;
        }
        let present = types.optional.then(|| quote! {
            fn present<'de, D, T>(deserializer: D) -> ::core::result::Result<Option<T>, D::Error>
            where D: ::berm_lang::serde::Deserializer<'de>, T: ::berm_lang::serde::Deserialize<'de> {
                T::deserialize(deserializer).map(Some)
            }
        });
        let scalar = needs_scalar.then(|| quote! {
            fn __crab_openapi_scalar(value: &::berm_lang::serde_json::Value) -> ::berm_lang::Result<::alloc::string::String> {
                match value {
                    ::berm_lang::serde_json::Value::String(value) => Ok(value.clone()),
                    ::berm_lang::serde_json::Value::Bool(_) | ::berm_lang::serde_json::Value::Number(_) => Ok(::alloc::string::ToString::to_string(value)),
                    _ => Err("expected a scalar parameter".into()),
                }
            }
        });
        let definitions = types.definitions;
        let tracked = self
            .path
            .to_str()
            .ok_or_else(|| Self::error("spec path is not UTF-8"))?;
        let tools_cfg = config.tools_cfg.as_ref().map(|cfg| quote!(#[#cfg]));
        let tracked_env = config
            .spec_env
            .as_ref()
            .map(|name| quote! { const _: &str = env!(#name); });
        let tool_module = config.tools.then(|| {
            quote! {
                #tools_cfg
                pub mod tools { #(#tools)* }
            }
        });
        Ok(quote! {
            const _: &str = include_str!(#tracked);
            #tracked_env
            pub mod types {
                #(#definitions)*
                #present
            }
            pub mod api {
                use crate::types;
                #scalar
                #(#functions)*
            }
            #tool_module
        })
    }

    fn operation(
        &self,
        config: &Config,
        name: &syn::Ident,
        selector: &str,
        types: &mut Types,
    ) -> syn::Result<(TokenStream, TokenStream, bool)> {
        let (method, route) = selector
            .split_once(' ')
            .ok_or_else(|| Self::error("use METHOD /path"))?;
        let item = self
            .spec
            .paths
            .as_ref()
            .and_then(|paths| paths.get(route))
            .ok_or_else(|| Self::error("path not found"))?;
        if item.reference.is_some() {
            return Err(Self::error("path-item references are not supported"));
        }
        let operation = item
            .methods()
            .into_iter()
            .find(|(m, _)| m.as_str() == method)
            .map(|(_, op)| op)
            .ok_or_else(|| Self::error("method not found"))?;
        if operation.deprecated == Some(true) {
            return Err(Self::error("operation is deprecated"));
        }
        let server = operation
            .servers
            .first()
            .or_else(|| item.servers.first())
            .or_else(|| self.spec.servers.first());
        let base = config
            .base_url
            .as_ref()
            .map(|s| s.value())
            .or_else(|| server.map(|s| s.url.clone()))
            .ok_or_else(|| Self::error("missing server URL or base_url"))?;
        if !(base.starts_with("https://") || base.starts_with("http://"))
            || base.contains(['{', '}', '?', '#'])
        {
            return Err(Self::error(
                "base URL must be an absolute HTTP URL without templates, query or fragment",
            ));
        }
        let mut parameters: Vec<(Parameter, Value)> = Vec::new();
        let raw_item = &self.source["paths"][route];
        let raw_operation = &raw_item[method.to_ascii_lowercase()];
        for parameter in raw_item["parameters"]
            .as_array()
            .into_iter()
            .flatten()
            .chain(raw_operation["parameters"].as_array().into_iter().flatten())
        {
            let resolved = self.reference(parameter, &mut Vec::new())?;
            let parameter: Parameter =
                serde_json::from_value(resolved.clone()).map_err(Self::error)?;
            let schema = resolved["schema"].clone();
            if let Some(i) = parameters.iter().position(|(p, _)| {
                p.location == parameter.location
                    && (p.name == parameter.name
                        || (p.location == ParameterIn::Header
                            && p.name.eq_ignore_ascii_case(&parameter.name)))
            }) {
                parameters[i] = (parameter, schema);
            } else {
                parameters.push((parameter, schema));
            }
        }
        let stem = name.to_string().to_upper_camel_case();
        let input_name = format!("{stem}Input");
        let mut properties = serde_json::Map::new();
        let mut required = Vec::new();
        let mut query = Vec::new();
        let mut headers = Vec::new();
        let mut path_params = BTreeSet::new();
        for (location, group) in [
            (ParameterIn::Path, "path"),
            (ParameterIn::Query, "query"),
            (ParameterIn::Header, "headers"),
        ] {
            let mut fields = serde_json::Map::new();
            let mut required_fields = Vec::new();
            for (parameter, raw) in parameters.iter().filter(|(p, _)| p.location == location) {
                if location == ParameterIn::Header
                    && ["accept", "content-type", "authorization"]
                        .contains(&parameter.name.to_ascii_lowercase().as_str())
                {
                    continue;
                }
                if raw.is_null() {
                    return Err(Self::error(format!(
                        "{}: content parameters are not supported",
                        parameter.name
                    )));
                }
                let raw = self.schema(raw, &mut Vec::new())?;
                let shape = Self::parameter_shape(&raw)
                    .map_err(|error| Self::error(format!("{}: {error}", parameter.name)))?;
                let expected_style = if location == ParameterIn::Query {
                    ParameterStyle::Form
                } else {
                    ParameterStyle::Simple
                };
                let style = parameter.style.unwrap_or(expected_style);
                let deep = style == ParameterStyle::DeepObject
                    && location == ParameterIn::Query
                    && shape == ParameterShape::Object
                    && parameter.explode == Some(true);
                let object = shape == ParameterShape::Object;
                if (!deep && style != expected_style)
                    || (object
                        && (location != ParameterIn::Query || parameter.explode == Some(false)))
                    || parameter.allow_reserved == Some(true)
                {
                    return Err(Self::error(format!(
                        "{}: unsupported parameter encoding",
                        parameter.name
                    )));
                }
                if location == ParameterIn::Path
                    && (parameter.required != Some(true) || shape != ParameterShape::Scalar)
                {
                    return Err(Self::error("path parameters must be required scalars"));
                }
                let wire = &parameter.name;
                let field = schema::identifier(wire)?;
                let access = quote!(group.#field);
                let is_required = parameter.required == Some(true);
                let mut raw = raw;
                if let Some(doc) = &parameter.description {
                    raw["description"] = Value::String(doc.clone());
                }
                fields.insert(wire.clone(), raw);
                if is_required {
                    required_fields.push(wire.clone());
                }
                let value = quote!(::berm_lang::serde_json::to_value(value)?);
                let write = if location == ParameterIn::Query {
                    let explode = parameter.explode.unwrap_or(true);
                    if shape == ParameterShape::Object {
                        quote! {
                            let value = #value;
                            if let Some(values) = value.as_object() {
                                for (key, value) in values {
                                    if !value.is_null() {
                                        let key = if #deep { ::alloc::format!("{}[{}]", #wire, key) } else { key.clone() };
                                        url.query_pairs_mut().append_pair(&key, &__crab_openapi_scalar(value)?);
                                    }
                                }
                            }
                        }
                    } else {
                        quote! {
                            let value = #value;
                            if let Some(values) = value.as_array() {
                                if #explode {
                                    for value in values { url.query_pairs_mut().append_pair(#wire, &__crab_openapi_scalar(value)?); }
                                } else {
                                    let values = values.iter().map(|value| {
                                        __crab_openapi_scalar(value).map(|value| ::url::form_urlencoded::byte_serialize(value.as_bytes()).collect::<::alloc::string::String>())
                                    }).collect::<::berm_lang::Result<::alloc::vec::Vec<_>>>()?;
                                    let key = ::url::form_urlencoded::byte_serialize(#wire.as_bytes()).collect::<::alloc::string::String>();
                                    let previous = url.query().unwrap_or("");
                                    let separator = if previous.is_empty() { "" } else { "&" };
                                    let query = ::alloc::format!("{previous}{separator}{key}={}", values.join(","));
                                    url.set_query(Some(&query));
                                }
                            } else if !value.is_null() { url.query_pairs_mut().append_pair(#wire, &__crab_openapi_scalar(&value)?); }
                        }
                    }
                } else if location == ParameterIn::Header {
                    quote! {
                        let value = #value;
                        let value = if let Some(values) = value.as_array() {
                            values.iter().map(__crab_openapi_scalar).collect::<::berm_lang::Result<::alloc::vec::Vec<_>>>()?.join(",")
                        } else { __crab_openapi_scalar(&value)? };
                        headers.push((::alloc::string::String::from(#wire), value));
                    }
                } else {
                    path_params.insert(wire.clone());
                    TokenStream::new()
                };
                let write = if is_required {
                    quote! { { let value = &#access; #write } }
                } else {
                    quote! { if let Some(value) = &#access { #write } }
                };
                if location == ParameterIn::Query {
                    query.push(write);
                } else if location == ParameterIn::Header {
                    headers.push(write);
                }
            }
            if !fields.is_empty() {
                let optional = required_fields.is_empty();
                properties.insert(group.to_owned(), json!({"type":"object", "properties":fields,"required":required_fields,"additionalProperties":false}));
                if !optional {
                    required.push(group.to_owned());
                }
                if location != ParameterIn::Path {
                    let writes = if location == ParameterIn::Query {
                        &mut query
                    } else {
                        &mut headers
                    };
                    let statements = std::mem::take(writes);
                    let group = format_ident!("{group}");
                    writes.push(if optional {
                        quote! { if let Some(group) = &input.#group { #(#statements)* } }
                    } else {
                        quote! { { let group = &input.#group; #(#statements)* } }
                    });
                }
            }
        }
        if parameters
            .iter()
            .any(|(p, _)| p.location == ParameterIn::Cookie)
        {
            return Err(Self::error("cookie parameters are not supported"));
        }
        let mut segments = Vec::new();
        let mut used_path_params = BTreeSet::new();
        for segment in route
            .strip_prefix('/')
            .ok_or_else(|| Self::error("path must start with /"))?
            .split('/')
        {
            if segment.starts_with('{') && segment.ends_with('}') {
                let wire = &segment[1..segment.len() - 1];
                if !path_params.contains(wire) {
                    return Err(Self::error(format!("missing path parameter {wire}")));
                }
                used_path_params.insert(wire.to_owned());
                let field = schema::identifier(wire)?;
                segments.push(quote! {
                    let value = __crab_openapi_scalar(&::berm_lang::serde_json::to_value(&input.path.#field)?)?;
                    if value == "." || value == ".." { return Err("dot path segments are not supported".into()); }
                    url.path_segments_mut().map_err(|_| ::berm_lang::Error::from("invalid base URL"))?.push(&value);
                });
            } else {
                if segment.contains(['{', '}']) {
                    return Err(Self::error("path templates must occupy a whole segment"));
                }
                segments.push(quote! { url.path_segments_mut().map_err(|_| ::berm_lang::Error::from("invalid base URL"))?.push(#segment); });
            }
        }
        if path_params != used_path_params {
            return Err(Self::error("path parameter absent from template"));
        }
        let mut body = quote!(::alloc::vec::Vec::<u8>::new());
        if operation.request_body.is_some() {
            let raw = self.reference(&raw_operation["requestBody"], &mut Vec::new())?;
            let request_body: oas3::spec::RequestBody =
                serde_json::from_value(raw.clone()).map_err(Self::error)?;
            let schema = raw
                .pointer("/content/application~1json/schema")
                .ok_or_else(|| {
                    Self::error("only application/json request bodies with a schema are supported")
                })?;
            properties.insert("body".to_owned(), self.schema(schema, &mut Vec::new())?);
            body = if request_body.required == Some(true) {
                required.push("body".to_owned());
                quote! { { headers.push(("Content-Type".into(), "application/json".into())); ::berm_lang::serde_json::to_vec(&input.body)? } }
            } else {
                quote! { if let Some(body) = &input.body { headers.push(("Content-Type".into(), "application/json".into())); ::berm_lang::serde_json::to_vec(body)? } else { ::alloc::vec::Vec::new() } }
            };
        }
        let input = types.object(&input_name, &json!({"type":"object", "properties":properties,"required":required,"additionalProperties":false}))?;
        let description = operation
            .summary
            .as_deref()
            .filter(|text| !text.trim().is_empty())
            .or_else(|| {
                operation
                    .description
                    .as_deref()
                    .filter(|text| !text.trim().is_empty())
            })
            .unwrap_or(selector);
        let client = &config.client;
        let mutable_headers =
            (!headers.is_empty() || operation.request_body.is_some()).then(|| quote!(mut));
        let response = config.response.as_ref().map_or_else(
            || quote!(Ok(response)),
            |handler| quote!(#handler(response)),
        );
        let function = quote! {
            #[doc = #description]
            pub fn #name(input: &types::#input) -> ::berm_lang::Result<::berm_lang::serde_json::Value> {
                let _ = input;
                let client = &(#client);
                let mut url = ::url::Url::parse(#base).map_err(|e| ::alloc::format!("invalid base URL: {e}"))?;
                url.path_segments_mut().map_err(|_| ::berm_lang::Error::from("invalid base URL"))?.pop_if_empty();
                #(#segments)*
                #(#query)*
                let #mutable_headers headers: ::alloc::vec::Vec<(::alloc::string::String, ::alloc::string::String)> = client.headers.iter().map(|(k,v)| ((*k).into(), (*v).into())).collect();
                #(#headers)*
                let body = #body;
                let headers: ::alloc::vec::Vec<(&str, &str)> = headers.iter().map(|(k,v)| (k.as_str(), v.as_str())).collect();
                let response = ::berm_lang::http::Client { name: client.name, headers: &headers }.request(#method, url.as_str(), &body)?;
                let response = if response.body.is_empty() { ::berm_lang::serde_json::Value::Null } else { response.read(client.name)? };
                #response
            }
        };
        let tool = quote! {
            #[doc = #description]
            #[::berm_lang::tool]
            pub fn #name(input: crate::types::#input) -> ::berm_lang::Result<::berm_lang::serde_json::Value> {
                crate::api::#name(&input)
            }
        };
        Ok((
            function,
            tool,
            !query.is_empty()
                || !headers.is_empty()
                || parameters
                    .iter()
                    .any(|(p, _)| p.location == ParameterIn::Path),
        ))
    }

    fn parameter_shape(schema: &Value) -> syn::Result<ParameterShape> {
        if let Some(variants) = schema["allOf"].as_array() {
            let mut shape = None;
            for variant in variants {
                if ["type", "allOf", "anyOf", "oneOf", "enum"]
                    .iter()
                    .all(|key| variant.get(key).is_none())
                {
                    continue;
                }
                let next = Self::parameter_shape(variant)?;
                if shape.is_some_and(|shape| shape != next) {
                    return Err(Self::error("incompatible parameter shapes in allOf"));
                }
                shape = Some(next);
            }
            if let Some(shape) = shape {
                return Ok(shape);
            }
        }
        if let Some(variants) = schema["anyOf"]
            .as_array()
            .or_else(|| schema["oneOf"].as_array())
        {
            let shapes = variants
                .iter()
                .filter(|v| v["type"] != "null")
                .map(Self::parameter_shape)
                .collect::<syn::Result<Vec<_>>>()?;
            if let Some(first) = shapes.first()
                && shapes.iter().all(|shape| shape == first)
            {
                return Ok(*first);
            }
        }
        if let Some(variants) = schema["type"].as_array() {
            let variants: Vec<_> = variants.iter().filter(|v| *v != "null").collect();
            if variants.len() == 1 {
                let mut schema = schema.clone();
                schema["type"] = variants[0].clone();
                return Self::parameter_shape(&schema);
            }
        }
        if schema.get("type").is_none()
            && let Some(values) = schema["enum"].as_array()
            && values.iter().any(|value| !value.is_null())
            && values.iter().all(|value| {
                value.is_null() || value.is_string() || value.is_number() || value.is_boolean()
            })
        {
            return Ok(ParameterShape::Scalar);
        }
        match schema["type"].as_str() {
            Some("string" | "integer" | "number" | "boolean") => Ok(ParameterShape::Scalar),
            Some("array") if Self::parameter_shape(&schema["items"])? == ParameterShape::Scalar => {
                Ok(ParameterShape::Array)
            }
            Some("object") => {
                for property in schema["properties"]
                    .as_object()
                    .into_iter()
                    .flat_map(|map| map.values())
                    .chain(
                        schema
                            .get("additionalProperties")
                            .filter(|value| value.is_object()),
                    )
                {
                    if Self::parameter_shape(property)? != ParameterShape::Scalar {
                        return Err(Self::error("object parameters must have scalar properties"));
                    }
                }
                if schema["additionalProperties"] == true
                    || (schema.get("properties").is_none()
                        && schema.get("additionalProperties").is_none())
                {
                    return Err(Self::error(
                        "object parameters must declare scalar properties",
                    ));
                }
                Ok(ParameterShape::Object)
            }
            _ => Err(Self::error(
                "parameters must be scalars, scalar arrays or flat objects",
            )),
        }
    }

    fn reference(&self, value: &Value, stack: &mut Vec<String>) -> syn::Result<Value> {
        let Some(reference) = value["$ref"].as_str() else {
            return Ok(value.clone());
        };
        let pointer = reference
            .strip_prefix('#')
            .ok_or_else(|| Self::error("external references are not supported"))?;
        if stack.iter().any(|r| r == reference) {
            return Err(Self::error(format!("recursive reference {reference}")));
        }
        stack.push(reference.to_owned());
        let target = self
            .source
            .pointer(pointer)
            .ok_or_else(|| Self::error(format!("unresolved reference {reference}")))?;
        let resolved = self.reference(target, stack)?;
        stack.pop();
        Ok(resolved)
    }

    fn schema(&self, value: &Value, stack: &mut Vec<(String, String, bool)>) -> syn::Result<Value> {
        let mut value = value.clone();
        if let Some(reference) = value["$ref"].as_str() {
            let reference = reference.to_owned();
            if let Some((_, id, recursive)) =
                stack.iter_mut().find(|(seen, _, _)| seen == &reference)
            {
                *recursive = true;
                value["$ref"] = Value::String(id.clone());
                return Ok(value);
            }
            let pointer = reference
                .strip_prefix('#')
                .ok_or_else(|| Self::error("external schema references are not supported"))?;
            let index = self.next_schema_id.get();
            self.next_schema_id.set(index + 1);
            let id = format!("urn:crab-openapi:schema:{index}");
            stack.push((reference.clone(), id.clone(), false));
            let target = self
                .source
                .pointer(pointer)
                .ok_or_else(|| Self::error(format!("unresolved schema {reference}")))?;
            let mut target = self.schema(target, stack)?;
            let (_, _, recursive) = stack.pop().unwrap();
            if recursive {
                target["$id"] = Value::String(id);
            }
            value.as_object_mut().unwrap().remove("$ref");
            if value.as_object().unwrap().is_empty() {
                return Ok(target);
            }
            return Ok(json!({"allOf":[target, self.schema(&value, stack)?]}));
        }
        if let Some(object) = value.as_object_mut() {
            for (key, nested) in object.iter_mut() {
                match key.as_str() {
                    "properties" | "patternProperties" | "$defs" | "dependentSchemas" => {
                        if let Some(map) = nested.as_object_mut() {
                            for value in map.values_mut() {
                                *value = self.schema(value, stack)?;
                            }
                        }
                    }
                    "anyOf" | "oneOf" | "allOf" | "prefixItems" => {
                        if let Some(array) = nested.as_array_mut() {
                            for value in array {
                                *value = self.schema(value, stack)?;
                            }
                        }
                    }
                    "items"
                    | "additionalProperties"
                    | "contains"
                    | "not"
                    | "if"
                    | "then"
                    | "else"
                    | "unevaluatedProperties"
                    | "unevaluatedItems"
                    | "propertyNames" => *nested = self.schema(nested, stack)?,
                    "$id" | "$dynamicRef" | "$anchor" | "$dynamicAnchor" => {
                        return Err(Self::error(format!("unsupported schema keyword {key}")));
                    }
                    _ => {}
                }
            }
        }
        Ok(value)
    }

    fn error(message: impl std::fmt::Display) -> syn::Error {
        syn::Error::new(Span::call_site(), message.to_string())
    }
}
