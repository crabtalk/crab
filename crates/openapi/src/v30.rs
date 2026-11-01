use proc_macro2::Span;
use serde_json::{Value, json};

pub(crate) fn normalize(document: &mut Value) -> syn::Result<()> {
    if let Some(components) = document["components"].as_object_mut() {
        for (name, entries) in components {
            if let Some(entries) = entries.as_object_mut() {
                for entry in entries.values_mut() {
                    match name.as_str() {
                        "schemas" => schema(entry)?,
                        "parameters" | "headers" => parameter(entry)?,
                        "requestBodies" => content(entry)?,
                        "responses" => response(entry)?,
                        "callbacks" => callback(entry)?,
                        _ => {}
                    }
                }
            }
        }
    }
    if let Some(paths) = document["paths"].as_object_mut() {
        for (path, item) in paths {
            if !path.starts_with("x-") {
                path_item(item)?;
            }
        }
    }
    document["openapi"] = json!("3.1.0");
    Ok(())
}

fn path_item(item: &mut Value) -> syn::Result<()> {
    if let Some(parameters) = item.get_mut("parameters").and_then(Value::as_array_mut) {
        for value in parameters {
            parameter(value)?;
        }
    }
    for method in [
        "get", "put", "post", "delete", "options", "head", "patch", "trace",
    ] {
        let Some(operation) = item.get_mut(method) else {
            continue;
        };
        if let Some(parameters) = operation
            .get_mut("parameters")
            .and_then(Value::as_array_mut)
        {
            for value in parameters {
                parameter(value)?;
            }
        }
        if let Some(body) = operation.get_mut("requestBody") {
            content(body)?;
        }
        if let Some(responses) = operation
            .get_mut("responses")
            .and_then(Value::as_object_mut)
        {
            for (status, value) in responses {
                if !status.starts_with("x-") {
                    response(value)?;
                }
            }
        }
        if let Some(callbacks) = operation
            .get_mut("callbacks")
            .and_then(Value::as_object_mut)
        {
            for value in callbacks.values_mut() {
                callback(value)?;
            }
        }
    }
    Ok(())
}

fn callback(value: &mut Value) -> syn::Result<()> {
    if let Some(paths) = value.as_object_mut() {
        for (name, value) in paths {
            if !name.starts_with("x-") {
                path_item(value)?;
            }
        }
    }
    Ok(())
}

fn parameter(value: &mut Value) -> syn::Result<()> {
    if let Some(value) = value.get_mut("schema") {
        schema(value)?;
    }
    content(value)
}

fn response(value: &mut Value) -> syn::Result<()> {
    if let Some(headers) = value.get_mut("headers").and_then(Value::as_object_mut) {
        for header in headers.values_mut() {
            parameter(header)?;
        }
    }
    content(value)
}

fn content(value: &mut Value) -> syn::Result<()> {
    if let Some(media) = value.get_mut("content").and_then(Value::as_object_mut) {
        for media in media.values_mut() {
            if let Some(schema_value) = media.get_mut("schema") {
                schema(schema_value)?;
            }
            if let Some(encodings) = media.get_mut("encoding").and_then(Value::as_object_mut) {
                for encoding in encodings.values_mut() {
                    if let Some(headers) =
                        encoding.get_mut("headers").and_then(Value::as_object_mut)
                    {
                        for header in headers.values_mut() {
                            parameter(header)?;
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

fn schema(value: &mut Value) -> syn::Result<()> {
    let Some(object) = value.as_object_mut() else {
        return Ok(());
    };
    if object.contains_key("$ref") {
        object.retain(|key, _| key == "$ref");
        return Ok(());
    }
    if object.remove("nullable") == Some(Value::Bool(true))
        && let Some(ty) = object
            .get("type")
            .and_then(Value::as_str)
            .map(str::to_owned)
    {
        object.insert("type".into(), json!([ty, "null"]));
    }
    for (exclusive, bound) in [
        ("exclusiveMinimum", "minimum"),
        ("exclusiveMaximum", "maximum"),
    ] {
        if let Some(flag) = object.remove(exclusive)
            && flag == true
        {
            let limit = object.remove(bound).ok_or_else(|| {
                syn::Error::new(
                    Span::call_site(),
                    format!("OpenAPI 3.0 {exclusive} requires {bound}"),
                )
            })?;
            object.insert(exclusive.into(), limit);
        }
    }
    if let Some(example) = object.remove("example") {
        object.entry("examples").or_insert(json!([example]));
    }
    if let Some(properties) = object.get_mut("properties").and_then(Value::as_object_mut) {
        for property in properties.values_mut() {
            schema(property)?;
        }
    }
    for key in ["items", "additionalProperties", "not"] {
        if let Some(nested) = object.get_mut(key) {
            schema(nested)?;
        }
    }
    for key in ["allOf", "anyOf", "oneOf"] {
        if let Some(nested) = object.get_mut(key).and_then(Value::as_array_mut) {
            for nested in nested {
                schema(nested)?;
            }
        }
    }
    Ok(())
}
