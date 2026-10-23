//! One MCP endpoint over every deployed program.

use crate::host::Service;
use berm_api::Output;
use rmcp::{
    ErrorData as McpError, ServerHandler,
    model::{
        CallToolRequestParams, CallToolResult, ContentBlock, Implementation, ListResourcesResult,
        ListToolsResult, Meta, PaginatedRequestParams, ReadResourceRequestParams,
        ReadResourceResult, Resource, ResourceContents, ServerCapabilities, ServerInfo, Tool,
        ToolAnnotations,
    },
    service::{NotificationContext, RequestContext, RoleServer},
};
use std::sync::Arc;

/// A server has one `instructions`, paid on every turn; each program's README
/// is a resource the model reads when it needs it.
const INSTRUCTIONS: &str = "Tools are named `{program}.{tool}`. Each program with a README \
    publishes it as a `berm://{program}/readme` resource; read it before choosing among one \
    program's tools.";

const README_SCHEME: &str = "berm://";
const README_PATH: &str = "/readme";
/// The `_meta` key a listed tool carries its terms under.
const TERMS: &str = "terms";

pub struct Mcp {
    service: Arc<Service>,
}

impl Mcp {
    pub fn new(service: Arc<Service>) -> Self {
        Self { service }
    }
}

impl ServerHandler for Mcp {
    fn get_info(&self) -> ServerInfo {
        let capabilities = ServerCapabilities::builder()
            .enable_tools()
            .enable_tool_list_changed()
            .enable_resources()
            .enable_resources_list_changed()
            .build();

        // Named here rather than from rmcp's build environment, which would
        // report rmcp.
        ServerInfo::new(capabilities)
            .with_server_info(Implementation::new(
                env!("CARGO_PKG_NAME"),
                env!("CARGO_PKG_VERSION"),
            ))
            .with_instructions(INSTRUCTIONS)
    }

    /// Deploying changes the tool set under clients already holding a list, so
    /// each session forwards that until its peer goes away.
    async fn on_initialized(&self, context: NotificationContext<RoleServer>) {
        let mut changed = self.service.subscribe();
        let peer = context.peer.clone();
        tokio::spawn(async move {
            while changed.recv().await.is_ok() {
                if peer.notify_tool_list_changed().await.is_err() {
                    break;
                }
                let _ = peer.notify_resource_list_changed().await;
            }
        });
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        let tools = self
            .service
            .list()
            .iter()
            .filter(|deployed| !self.service.host.stopped(&deployed.name))
            .flat_map(|deployed| {
                let terms = self.service.host.terms(&deployed.name).unwrap_or_default();
                deployed
                    .manifest()
                    .tools
                    .iter()
                    .filter(|tool| tool.anonymous)
                    .map(move |tool| {
                        let mut listed = Tool::new(
                            format!("{}.{}", deployed.name, tool.name),
                            tool.description.clone(),
                            Arc::new(tool.parameters.as_object().cloned().unwrap_or_default()),
                        );
                        if let Some(returns) = tool.returns.as_ref().and_then(|v| v.as_object()) {
                            listed = listed.with_raw_output_schema(Arc::new(returns.clone()));
                        }
                        if tool.query {
                            listed = listed.annotate(ToolAnnotations::new().read_only(true));
                        }
                        if let Some(held) = terms.tools.get(&tool.name) {
                            let mut meta = Meta::new();
                            meta.insert(TERMS.into(), serde_json::json!(held));
                            listed = listed.with_meta(meta);
                        }
                        listed
                    })
            })
            .collect();
        Ok(ListToolsResult::with_all_items(tools))
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        let Some((program, tool)) = request.name.split_once('.') else {
            return Err(McpError::invalid_params(
                format!(
                    "tool {:?} is not named `{{program}}.{{tool}}`",
                    request.name
                ),
                None,
            ));
        };

        // A program reads its arguments as the JSON its manifest advertised a
        // schema for, so the object goes across as it arrived.
        let args = match &request.arguments {
            Some(arguments) => serde_json::to_vec(arguments)
                .map_err(|error| McpError::invalid_params(error.to_string(), None))?,
            None => Vec::new(),
        };

        if !self
            .service
            .get(program)
            .is_some_and(|deployed| deployed.reached_anonymously(tool))
        {
            return Err(McpError::invalid_params(
                format!("{program} has no tool named {tool:?} that takes anonymous calls"),
                None,
            ));
        }

        // A tool listed with an output schema answers with structured content
        // as well, which MCP requires of it.
        let structured = self.service.get(program).is_some_and(|deployed| {
            deployed
                .manifest()
                .tools
                .iter()
                .any(|spec| spec.name == tool && spec.returns.is_some())
        });

        match self.service.call(program, tool, args).await {
            Ok(Output::Done(result)) if structured => match serde_json::from_str(&result) {
                Ok(value) => Ok(CallToolResult::structured(value)),
                Err(error) => Ok(CallToolResult::error(vec![ContentBlock::text(format!(
                    "{program}.{tool} declares a result schema and answered something that is not JSON: {error}"
                ))])),
            },
            Ok(Output::Done(result)) => {
                Ok(CallToolResult::success(vec![ContentBlock::text(result)]))
            }
            // The program ran and reported failure. That is a tool result the
            // model should see and react to, not a protocol error.
            Ok(Output::Failed(failure)) => {
                Ok(CallToolResult::error(vec![ContentBlock::text(failure)]))
            }
            Err(error) => Err(McpError::internal_error(format!("{error:#}"), None)),
        }
    }

    async fn list_resources(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, McpError> {
        let resources = self
            .service
            .list()
            .iter()
            .filter(|deployed| !deployed.manifest().readme.is_empty())
            .map(|deployed| {
                Resource::new(
                    format!("{README_SCHEME}{}{README_PATH}", deployed.name),
                    format!("{} README", deployed.name),
                )
                .with_description(format!("{}'s README.", deployed.name))
                .with_mime_type("text/markdown")
            })
            .collect();
        Ok(ListResourcesResult::with_all_items(resources))
    }

    async fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResult, McpError> {
        let name = request
            .uri
            .strip_prefix(README_SCHEME)
            .and_then(|rest| rest.strip_suffix(README_PATH))
            .ok_or_else(|| McpError::resource_not_found(request.uri.clone(), None))?;

        let deployed = self
            .service
            .get(name)
            .ok_or_else(|| McpError::resource_not_found(request.uri.clone(), None))?;

        Ok(ReadResourceResult::new(vec![ResourceContents::text(
            deployed.manifest().readme.clone(),
            request.uri,
        )]))
    }
}
