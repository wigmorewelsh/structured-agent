mod file;

use file::WorkspaceFile;
use rmcp::{
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::*,
    schemars, tool, tool_handler, tool_router, ErrorData as McpError, ServerHandler,
};
use std::path::PathBuf;

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct ReadFileRequest {
    #[schemars(description = "Path to the file relative to the workspace root")]
    pub path: String,
    #[schemars(description = "Optional symbol name to retrieve its full definition")]
    pub symbol: Option<String>,
}

#[derive(Clone)]
pub struct WorkspaceServer {
    workspace_root: PathBuf,
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl WorkspaceServer {
    pub fn new(workspace_root: PathBuf) -> Self {
        Self {
            workspace_root,
            tool_router: Self::tool_router(),
        }
    }

    #[tool(
        description = "Read a file from the workspace. Returns a tree-sitter outline showing top-level symbols (functions, types, structs, etc.) with their line numbers. If a symbol name is provided, returns the full definition of that symbol instead."
    )]
    pub fn read_file(
        &self,
        Parameters(request): Parameters<ReadFileRequest>,
    ) -> Result<CallToolResult, McpError> {
        let file = WorkspaceFile::open(&self.workspace_root, &request.path)?;

        let content = match &request.symbol {
            Some(symbol_name) => file.get_symbol(symbol_name)?,
            None => file.get_outline()?,
        };

        Ok(CallToolResult::success(vec![Content::text(content)]))
    }
}

#[tool_handler]
impl ServerHandler for WorkspaceServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::V_2024_11_05,
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            server_info: Implementation::from_build_env(),
            instructions: Some(
                "This server provides structured file reading for a workspace. \
                 Use the read_file tool to get an outline of symbols in a file, \
                 or retrieve specific symbol definitions."
                    .to_string(),
            ),
        }
    }
}
