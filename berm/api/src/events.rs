use crate::{ToolSpec, schema};
use anyhow::{Context, Result, bail, ensure};
use berm_abi::events::Page;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// MCP discovery metadata. Subscription filters exclude the host-owned cursor.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Definition {
    pub name: String,
    pub description: String,
    pub delivery: Vec<String>,
    pub input_schema: Value,
    pub payload_schema: Value,
}

#[derive(Debug, Clone)]
pub struct Source {
    pub tool: String,
    pub definition: Definition,
}

impl TryFrom<&ToolSpec> for Source {
    type Error = anyhow::Error;

    fn try_from(tool: &ToolSpec) -> Result<Self> {
        let name = tool
            .events
            .as_deref()
            .context("tool is not an event source")?;
        ensure!(
            !name.is_empty()
                && name.len() <= 128
                && name
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte)),
            "event names use 1–128 ASCII letters, digits, dots, underscores or hyphens"
        );
        ensure!(
            tool.query && !tool.cron && !tool.stripe && !tool.webhook,
            "an event source must be a read-only tool without another host hook"
        );
        let mut input = tool.parameters.clone();
        ensure!(
            input["type"] == "object",
            "event source arguments must be an object"
        );
        let properties = input["properties"]
            .as_object_mut()
            .context("event source needs a cursor argument")?;
        let cursor = properties
            .remove("cursor")
            .context("event source needs a cursor argument")?;
        ensure!(
            schema::nullable(&cursor).unwrap_or(&cursor)["type"] == "string",
            "event source cursor must be Option<String>"
        );
        ensure!(
            !input["required"]
                .as_array()
                .is_some_and(|keys| keys.iter().any(|key| key == "cursor")),
            "event source cursor must be optional"
        );
        input["additionalProperties"] = json!(false);

        let output = tool
            .returns
            .as_ref()
            .context("event source must return an event page")?;
        ensure!(
            output["type"] == "object" && output["properties"]["events"]["type"] == "array",
            "event source must return an event page"
        );
        let record = &output["properties"]["events"]["items"];
        for (object, fields) in [
            (
                output,
                &[
                    ("events", "array"),
                    ("cursor", "string"),
                    ("has_more", "boolean"),
                ][..],
            ),
            (
                record,
                &[("id", "string"), ("cursor", "string"), ("at", "integer")][..],
            ),
        ] {
            ensure!(
                object["type"] == "object",
                "event page and records must be objects"
            );
            for (field, kind) in fields {
                ensure!(
                    object["properties"][field]["type"] == *kind
                        && object["required"]
                            .as_array()
                            .is_some_and(|keys| keys.iter().any(|key| key == field)),
                    "event page field {field} must be a required {kind}"
                );
            }
        }
        ensure!(
            record["required"]
                .as_array()
                .is_some_and(|keys| keys.iter().any(|key| key == "data")),
            "event data must be required"
        );
        let payload = record["properties"]
            .get("data")
            .context("event source needs a payload schema")?
            .clone();
        Ok(Self {
            tool: tool.name.clone(),
            definition: Definition {
                name: name.to_owned(),
                description: tool.description.clone(),
                delivery: vec!["webhook".into()],
                input_schema: input,
                payload_schema: payload,
            },
        })
    }
}

impl Source {
    pub fn page(&self, body: &str, cursor: Option<&str>) -> Result<Page<Value>> {
        let page: Page<Value> = serde_json::from_str(body).context("Invalid event page")?;
        page.check(cursor).map_err(anyhow::Error::msg)?;
        Ok(page)
    }

    pub fn qualify(mut self, service: &str) -> Self {
        self.definition.name = format!("{service}.{}", self.definition.name);
        self
    }

    pub fn arguments(&self, filters: Value, cursor: Option<&str>) -> Result<Vec<u8>> {
        let Value::Object(mut fields) = filters else {
            bail!("event filters must be an object");
        };
        let properties = self.definition.input_schema["properties"]
            .as_object()
            .context("event filters have no schema")?;
        for key in fields.keys() {
            ensure!(properties.contains_key(key), "unknown event filter {key}");
        }
        if let Some(required) = self.definition.input_schema["required"].as_array() {
            for key in required.iter().filter_map(Value::as_str) {
                ensure!(fields.contains_key(key), "missing event filter {key}");
            }
        }
        if let Some(cursor) = cursor {
            fields.insert("cursor".into(), json!(cursor));
        }
        Ok(serde_json::to_vec(&fields)?)
    }
}
