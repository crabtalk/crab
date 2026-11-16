use crate::host::mcp::Mcp;
use axum::http::StatusCode;
use berm_api::{host::Caller, workflow::run::Failure};
use berm_host::{self, Denied};
use crabtalk_sdk::mcp::workflow::{self, Action};
use rmcp::{ErrorData as McpError, model::CallToolResult};
use serde_json::{Map, Value, json};

impl Mcp {
    pub(super) async fn workflow(
        &self,
        action: Action,
        arguments: Map<String, Value>,
        caller: Option<&Caller>,
    ) -> Result<CallToolResult, McpError> {
        let operator = Caller {
            id: "local".into(),
            key: None,
            app: None,
            user: None,
            client: None,
            agent: None,
        };
        let caller = caller.unwrap_or(&operator);
        let parameters = Value::Object(arguments);
        let result: anyhow::Result<Value> = async {
            match action {
                Action::Inspect => {
                    let request: workflow::Inspect =
                        serde_json::from_value(parameters).map_err(|error| {
                            berm_host::deny(StatusCode::BAD_REQUEST, error.to_string())
                        })?;
                    if request.workflow.is_some() && request.service.is_none() {
                        return Err(berm_host::deny(
                            StatusCode::BAD_REQUEST,
                            "workflow requires service",
                        )
                        .into());
                    }
                    if let (Some(service), Some(workflow)) = (&request.service, &request.workflow) {
                        return Ok(serde_json::to_value(self.node.host.inspect_workflow(
                            service,
                            workflow,
                            Some(caller),
                        )?)?);
                    }
                    let mut inspections = Vec::new();
                    for deployed in self.node.list() {
                        if self.node.host.stopped(&deployed.name)
                            || request
                                .service
                                .as_ref()
                                .is_some_and(|name| name.as_str() != deployed.name.as_ref())
                        {
                            continue;
                        }
                        for spec in &deployed.manifest.workflows {
                            match self.node.host.inspect_workflow(
                                &deployed.name,
                                &spec.entry.name,
                                Some(caller),
                            ) {
                                Ok(inspection) => inspections.push(inspection),
                                Err(error)
                                    if error.downcast_ref::<Denied>().is_some_and(|error| {
                                        matches!(error.status.as_u16(), 401 | 403 | 404)
                                    }) => {}
                                Err(error) => return Err(error),
                            }
                        }
                    }
                    Ok(json!({"workflows":inspections}))
                }
                Action::Run => {
                    let request: workflow::Run =
                        serde_json::from_value(parameters).map_err(|error| {
                            berm_host::deny(StatusCode::BAD_REQUEST, error.to_string())
                        })?;
                    Ok(serde_json::to_value(
                        self.node
                            .host
                            .start_local_workflow(
                                &request.service,
                                &request.workflow,
                                request.request,
                                caller.clone(),
                            )
                            .await?,
                    )?)
                }
                Action::Status | Action::Continue | Action::Cancel => {
                    let request: workflow::Control =
                        serde_json::from_value(parameters).map_err(|error| {
                            berm_host::deny(StatusCode::BAD_REQUEST, error.to_string())
                        })?;
                    let record = match action {
                        Action::Continue => {
                            self.node
                                .host
                                .continue_local_workflow(
                                    &request.service,
                                    &request.workflow,
                                    &request.id,
                                )
                                .await?
                        }
                        Action::Cancel => {
                            self.node
                                .host
                                .cancel_local_workflow(
                                    &request.service,
                                    &request.workflow,
                                    &request.id,
                                )
                                .await?
                        }
                        _ => {
                            self.node
                                .host
                                .workflow_record(&request.service, &request.workflow, &request.id)
                                .await?
                        }
                    };
                    Ok(serde_json::to_value(record)?)
                }
            }
        }
        .await;
        match result {
            Ok(value) => Ok(CallToolResult::structured(value)),
            Err(error) => {
                let denied = Denied::from(error);
                let failure = Failure::from(&denied);
                let mut body = match failure.body {
                    Value::Object(body) => body,
                    body => Map::from_iter([("body".into(), body)]),
                };
                body.insert("status".into(), json!(failure.status));
                if !failure.headers.is_empty() {
                    body.insert("headers".into(), json!(failure.headers));
                }
                Ok(CallToolResult::structured_error(Value::Object(body)))
            }
        }
    }
}
