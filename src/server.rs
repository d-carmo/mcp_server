use std::sync::Arc;

use rmcp::{
    ErrorData,
    ServerHandler,
    model::{
        CallToolRequestParams, CallToolResult, Content, ErrorCode, GetPromptResult,
        GetPromptRequestParams, Implementation, ListPromptsResult, ListResourcesResult,
        ListToolsResult, PaginatedRequestParams, Prompt, PromptArgument,
        PromptMessage as RmcpPromptMessage, PromptMessageRole, PromptsCapability,
        ProtocolVersion, RawResource, ReadResourceRequestParams, ReadResourceResult, Resource,
        ResourceContents, ResourcesCapability, ServerCapabilities, ServerInfo, Tool,
        ToolsCapability,
    },
    service::{RequestContext, RoleServer},
};

use crate::prompts::{McpPrompt, Role, all_prompts};
use crate::resources::{McpResource, all_resources};
use crate::tools::{McpTool, all_tools};

// ---------------------------------------------------------------------------
// McpServer
// ---------------------------------------------------------------------------

/// Central MCP server.
///
/// Holds all registered tools, resources, and prompts collected at startup via
/// the `inventory` crate, and implements rmcp's [`ServerHandler`] to serve them
/// over any transport (stdio or Streamable HTTP).
pub struct McpServer {
    tools: Vec<Box<dyn McpTool>>,
    resources: Vec<Box<dyn McpResource>>,
    prompts: Vec<Box<dyn McpPrompt>>,
}

impl McpServer {
    /// Collect all registered tools, resources, and prompts and return a ready
    /// server instance.
    pub fn new() -> Self {
        Self {
            tools: all_tools(),
            resources: all_resources(),
            prompts: all_prompts(),
        }
    }
}

impl Default for McpServer {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// ServerHandler implementation
// ---------------------------------------------------------------------------

impl ServerHandler for McpServer {
    fn get_info(&self) -> ServerInfo {
        let mut capabilities = ServerCapabilities::default();
        capabilities.tools = Some(ToolsCapability::default());
        capabilities.resources = Some(ResourcesCapability::default());
        capabilities.prompts = Some(PromptsCapability::default());
        ServerInfo::new(capabilities)
            .with_server_info(Implementation::new(
                env!("CARGO_PKG_NAME"),
                env!("CARGO_PKG_VERSION"),
            ))
            .with_protocol_version(ProtocolVersion::LATEST)
    }

    // --- Tools --------------------------------------------------------------

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        let tools: Vec<Tool> = self
            .tools
            .iter()
            .map(|t| Tool::new(t.name(), t.description(), Arc::new(t.schema())))
            .collect();

        Ok(ListToolsResult::with_all_items(tools))
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        let name = request.name.as_ref();
        let params = request.arguments.unwrap_or_default();

        match self.tools.iter().find(|t| t.name() == name) {
            Some(tool) => match tool.call(params).await {
                Ok(text) => Ok(CallToolResult::success(vec![Content::text(text)])),
                Err(e) => Ok(CallToolResult::error(vec![Content::text(format!("Error: {e}"))])),
            },
            None => Err(ErrorData {
                code: ErrorCode::METHOD_NOT_FOUND,
                message: format!("tool '{}' not found", name).into(),
                data: None,
            }),
        }
    }

    // --- Resources ----------------------------------------------------------

    async fn list_resources(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, ErrorData> {
        let resources: Vec<Resource> = self
            .resources
            .iter()
            .map(|r| {
                Resource::new(
                    RawResource {
                        uri: r.uri().to_string(),
                        name: r.name().to_string(),
                        title: None,
                        description: r.description().map(str::to_string),
                        mime_type: r.mime_type().map(str::to_string),
                        size: None,
                        icons: None,
                        meta: None,
                    },
                    None,
                )
            })
            .collect();

        Ok(ListResourcesResult::with_all_items(resources))
    }

    async fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResult, ErrorData> {
        match self.resources.iter().find(|r| r.uri() == request.uri) {
            Some(resource) => match resource.read().await {
                Ok(text) => Ok(ReadResourceResult::new(vec![ResourceContents::text(
                    text,
                    resource.uri(),
                )])),
                Err(e) => Err(ErrorData {
                    code: ErrorCode::INTERNAL_ERROR,
                    message: format!("failed to read '{}': {e}", request.uri).into(),
                    data: None,
                }),
            },
            None => Err(ErrorData {
                code: ErrorCode::RESOURCE_NOT_FOUND,
                message: format!("resource '{}' not found", request.uri).into(),
                data: None,
            }),
        }
    }

    // --- Prompts ------------------------------------------------------------

    async fn list_prompts(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListPromptsResult, ErrorData> {
        let prompts: Vec<Prompt> = self
            .prompts
            .iter()
            .map(|p| {
                let args: Vec<PromptArgument> = p
                    .arguments()
                    .into_iter()
                    .map(|a| {
                        let mut arg = PromptArgument::new(a.name).with_required(a.required);
                        if let Some(d) = a.description {
                            arg = arg.with_description(d);
                        }
                        arg
                    })
                    .collect();

                Prompt::new(p.name(), p.description(), Some(args))
            })
            .collect();

        Ok(ListPromptsResult::with_all_items(prompts))
    }

    async fn get_prompt(
        &self,
        request: GetPromptRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<GetPromptResult, ErrorData> {
        let args = request.arguments.unwrap_or_default();

        match self.prompts.iter().find(|p| p.name() == request.name) {
            Some(prompt) => match prompt.get(args).await {
                Ok(messages) => {
                    let rmcp_messages: Vec<RmcpPromptMessage> = messages
                        .into_iter()
                        .map(|m| {
                            RmcpPromptMessage::new_text(
                                match m.role {
                                    Role::User => PromptMessageRole::User,
                                    Role::Assistant => PromptMessageRole::Assistant,
                                },
                                m.text,
                            )
                        })
                        .collect();

                    let mut result = GetPromptResult::new(rmcp_messages);
                    if let Some(d) = prompt.description() {
                        result = result.with_description(d);
                    }
                    Ok(result)
                }
                Err(e) => Err(ErrorData {
                    code: ErrorCode::INTERNAL_ERROR,
                    message: format!("failed to render '{}': {e}", request.name).into(),
                    data: None,
                }),
            },
            None => Err(ErrorData {
                code: ErrorCode::METHOD_NOT_FOUND,
                message: format!("prompt '{}' not found", request.name).into(),
                data: None,
            }),
        }
    }
}
