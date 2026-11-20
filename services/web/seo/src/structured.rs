use crate::report::{Code, Finding, Level};
use berm_lang::{
    Schema,
    serde_json::{self, Value},
};
use scraper::Selector;
use std::collections::BTreeSet;
use webpage::page::Page;

const JSON_LD: &str = "application/ld+json";

#[derive(Debug, Schema)]
pub struct Block {
    pub index: usize,
    pub types: Vec<String>,
    pub error: Option<String>,
}

impl Block {
    pub(crate) fn inspect(page: &Page, findings: &mut Vec<Finding>) -> Vec<Self> {
        let mut blocks = Vec::new();
        for element in page
            .document
            .select(&Selector::parse("script[type]").expect("static selector"))
        {
            if !element
                .value()
                .attr("type")
                .unwrap_or_default()
                .trim()
                .eq_ignore_ascii_case(JSON_LD)
            {
                continue;
            }
            let mut types = BTreeSet::new();
            let mut error = None;
            match serde_json::from_str::<Value>(&element.text().collect::<String>()) {
                Err(cause) => error = Some(cause.to_string()),
                Ok(value) => {
                    if !value.is_object()
                        && !value
                            .as_array()
                            .is_some_and(|items| items.iter().all(Value::is_object))
                    {
                        error = Some(
                            "JSON-LD must have an object or array of objects at its root.".into(),
                        );
                    }
                    let mut pending = vec![&value];
                    while let Some(value) = pending.pop() {
                        match value {
                            Value::Object(object) => {
                                if let Some(kind) = object.get("@type") {
                                    match kind {
                                        Value::String(kind) if !kind.trim().is_empty() => {
                                            types.insert(kind.clone());
                                        }
                                        Value::Array(kinds)
                                            if !kinds.is_empty()
                                                && kinds.iter().all(|kind| {
                                                    kind.as_str()
                                                        .is_some_and(|kind| !kind.trim().is_empty())
                                                }) =>
                                        {
                                            types.extend(
                                                kinds
                                                    .iter()
                                                    .filter_map(Value::as_str)
                                                    .map(str::to_owned),
                                            );
                                        }
                                        _ => {
                                            error = Some("An @type must be a nonempty string or array of nonempty strings.".into());
                                        }
                                    }
                                }
                                if object.get("@graph").is_some_and(|graph| {
                                    !graph.is_object()
                                        && !graph
                                            .as_array()
                                            .is_some_and(|items| items.iter().all(Value::is_object))
                                }) {
                                    error = Some(
                                        "An @graph must be an object or array of objects.".into(),
                                    );
                                }
                                pending.extend(
                                    object
                                        .iter()
                                        .filter(|(key, _)| key.as_str() != "@context")
                                        .map(|(_, value)| value),
                                );
                            }
                            Value::Array(items) => pending.extend(items),
                            _ => {}
                        }
                    }
                }
            }
            blocks.push(Self {
                index: blocks.len() + 1,
                types: types.into_iter().collect(),
                error,
            });
        }
        if blocks.iter().any(|block| block.error.is_some()) {
            findings.push(Finding::new(Code::InvalidStructuredData, Level::Warning, "A JSON-LD block has invalid JSON or an invalid root, @type or @graph shape; see its error."));
        }
        blocks
    }
}
