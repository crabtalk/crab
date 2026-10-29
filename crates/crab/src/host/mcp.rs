//! One MCP endpoint over every deployed program: `search`, `call`, and each
//! program's README and IDL as a resource, as [`crabtalk_sdk::mcp`] lays them out. A
//! call carries no caller, so only `access = "anonymous"` tools are found or
//! called.

use crate::host::Service;
use berm_api::Output;
use crabtalk_sdk::mcp::{self, Found};
use rmcp::{
    ErrorData as McpError, ServerHandler,
    model::{
        CallToolRequestParams, CallToolResult, ContentBlock, Implementation,
        ListResourceTemplatesResult, ListToolsResult, PaginatedRequestParams,
        ReadResourceRequestParams, ReadResourceResult, ResourceContents, ResourceTemplate,
        ServerCapabilities, ServerInfo, Tool, ToolAnnotations,
    },
    service::{RequestContext, RoleServer},
};
use std::sync::Arc;

/// The most programs a search answers.
const FOUND: usize = 10;

pub struct Mcp {
    service: Arc<Service>,
}

impl Mcp {
    pub fn new(service: Arc<Service>) -> Self {
        Self { service }
    }

    /// The running programs with a tool that takes anonymous calls, each
    /// with those tools, matching `query`'s words: best first, a word in the
    /// name counting most, then in the description, then in a tool.
    fn search(&self, query: &str) -> Vec<Found> {
        let words: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
        let count = |text: &str| {
            let text = text.to_lowercase();
            words
                .iter()
                .filter(|word| text.contains(word.as_str()))
                .count()
        };
        let mut found: Vec<(usize, Found)> = self
            .service
            .list()
            .iter()
            .filter(|deployed| !self.service.host.stopped(&deployed.name))
            .filter_map(|deployed| {
                let manifest = &deployed.manifest;
                let tools: Vec<mcp::Tool> = manifest
                    .tools
                    .iter()
                    .filter(|tool| tool.anonymous)
                    .map(|tool| mcp::Tool {
                        name: tool.name.clone(),
                        title: tool.title.clone(),
                        description: tool.description.clone(),
                        price: None,
                    })
                    .collect();
                let score = 3 * count(&deployed.name)
                    + 2 * count(&manifest.description)
                    + tools
                        .iter()
                        .map(|tool| {
                            let title = tool.title.as_deref().unwrap_or_default();
                            count(&format!("{} {title} {}", tool.name, tool.description))
                        })
                        .sum::<usize>();
                (score > 0 && !tools.is_empty()).then(|| {
                    let found = Found {
                        service: deployed.name.to_string(),
                        description: manifest.description.clone(),
                        tools,
                    };
                    (score, found)
                })
            })
            .collect();
        found
            .sort_by(|(a, one), (b, other)| b.cmp(a).then_with(|| one.service.cmp(&other.service)));
        found
            .into_iter()
            .take(FOUND)
            .map(|(_, found)| found)
            .collect()
    }
}

impl ServerHandler for Mcp {
    fn get_info(&self) -> ServerInfo {
        let capabilities = ServerCapabilities::builder()
            .enable_tools()
            .enable_resources()
            .build();

        // crab's own name; rmcp's build environment would report rmcp.
        ServerInfo::new(capabilities)
            .with_server_info(Implementation::new(
                env!("CARGO_PKG_NAME"),
                env!("CARGO_PKG_VERSION"),
            ))
            .with_instructions(mcp::INSTRUCTIONS)
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        let search = Tool::new(
            mcp::SEARCH,
            mcp::SEARCH_DESCRIPTION,
            Arc::new(mcp::search_schema()),
        )
        .annotate(ToolAnnotations::new().read_only(true));
        let call = Tool::new(
            mcp::CALL,
            mcp::CALL_DESCRIPTION,
            Arc::new(mcp::call_schema()),
        );
        Ok(ListToolsResult::with_all_items(vec![search, call]))
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        let arguments = request.arguments.unwrap_or_default();
        let field = |key: &str| {
            arguments
                .get(key)
                .and_then(|value| value.as_str())
                .ok_or_else(|| McpError::invalid_params(format!("`{key}` is a string"), None))
        };
        match request.name.as_ref() {
            mcp::SEARCH => {
                let found = serde_json::to_string(&self.search(field("query")?))
                    .map_err(|error| McpError::internal_error(error.to_string(), None))?;
                return Ok(CallToolResult::success(vec![ContentBlock::text(found)]));
            }
            mcp::CALL => {}
            name => {
                return Err(McpError::invalid_params(
                    format!("no tool named {name:?}"),
                    None,
                ));
            }
        }
        let program = field("service")?;
        let tool = field("tool")?;
        if program.contains('/') {
            return Ok(CallToolResult::error(vec![ContentBlock::text(
                "the local host has no accounts: name a service it runs",
            )]));
        }
        if !self
            .service
            .get(program)
            .is_some_and(|deployed| deployed.reached_anonymously(tool))
        {
            return Ok(CallToolResult::error(vec![ContentBlock::text(format!(
                "{program} has no tool named {tool:?} that takes anonymous calls"
            ))]));
        }

        // A program reads its arguments as the JSON its manifest advertised a
        // schema for, so the object goes across as it arrived.
        let input = arguments
            .get("input")
            .cloned()
            .unwrap_or_else(|| serde_json::json!({}));
        let args = serde_json::to_vec(&input)
            .map_err(|error| McpError::invalid_params(error.to_string(), None))?;

        match self.service.call(program, tool, args).await {
            Ok(Output::Done(result)) => {
                Ok(CallToolResult::success(vec![ContentBlock::text(result)]))
            }
            // The program ran and reported failure: a tool result.
            Ok(Output::Failed(failure)) => {
                Ok(CallToolResult::error(vec![ContentBlock::text(failure)]))
            }
            Err(error) => Ok(CallToolResult::error(vec![ContentBlock::text(
                error.to_string(),
            )])),
        }
    }

    async fn list_resource_templates(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourceTemplatesResult, McpError> {
        let template = ResourceTemplate::new(mcp::TEMPLATE, "service")
            .with_description(mcp::TEMPLATE_DESCRIPTION);
        Ok(ListResourceTemplatesResult::with_all_items(vec![template]))
    }

    async fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResult, McpError> {
        let not_found = || McpError::resource_not_found(request.uri.clone(), None);
        let name = mcp::service(&request.uri).ok_or_else(not_found)?;
        let program = self.service.host.idl(name).map_err(|_| not_found())?;
        let idl = serde_json::to_string(&program)
            .map_err(|error| McpError::internal_error(error.to_string(), None))?;
        Ok(ReadResourceResult::new(vec![
            ResourceContents::text(program.readme, request.uri.clone())
                .with_mime_type("text/markdown"),
            ResourceContents::text(idl, request.uri).with_mime_type("application/json"),
        ]))
    }
}
