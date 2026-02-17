use crate::parser::{FileParser, LanguageType, Symbol};
use rmcp::{
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::*,
    schemars, tool, tool_handler, tool_router, ErrorData as McpError, ServerHandler,
};
use serde_json::json;
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

    fn resolve_path(&self, path: &str) -> Result<PathBuf, McpError> {
        let full_path = self.workspace_root.join(path);

        if !full_path.starts_with(&self.workspace_root) {
            return Err(McpError::invalid_params(
                "Path escapes workspace root".to_string(),
                Some(json!({"path": path})),
            ));
        }

        if !full_path.exists() {
            return Err(McpError::invalid_params(
                "File does not exist".to_string(),
                Some(json!({"path": path})),
            ));
        }

        if !full_path.is_file() {
            return Err(McpError::invalid_params(
                "Path is not a file".to_string(),
                Some(json!({"path": path})),
            ));
        }

        Ok(full_path)
    }

    fn format_outline(symbols: &[Symbol]) -> String {
        let mut output = String::new();

        for symbol in symbols {
            let display_name = if let Some(parent_start) = symbol.parent_start_line {
                let parent = symbols
                    .iter()
                    .find(|s| s.start_line == parent_start)
                    .map(|s| s.name.as_str())
                    .unwrap_or("");

                format!("{}::{}", parent, symbol.name)
            } else {
                symbol.name.clone()
            };

            let kind_display = if symbol.kind == "impl" {
                if let Some(ref trait_name) = symbol.trait_name {
                    format!("{} {} for {}", symbol.kind, trait_name, symbol.name)
                } else {
                    format!("{} {}", symbol.kind, symbol.name)
                }
            } else {
                format!("{} {}", symbol.kind, display_name)
            };

            output.push_str(&format!(
                "{}-{} {}\n",
                symbol.start_line, symbol.end_line, kind_display
            ));
        }

        output
    }

    #[tool(
        description = "Read a file from the workspace. Returns a tree-sitter outline showing top-level symbols (functions, types, structs, etc.) with their line numbers. If a symbol name is provided, returns the full definition of that symbol instead."
    )]
    pub fn read_file(
        &self,
        Parameters(request): Parameters<ReadFileRequest>,
    ) -> Result<CallToolResult, McpError> {
        let full_path = self.resolve_path(&request.path)?;

        let source = std::fs::read_to_string(&full_path).map_err(|e| {
            McpError::internal_error(
                format!("Failed to read file: {}", e),
                Some(json!({"path": &request.path})),
            )
        })?;

        let lang_type = LanguageType::from_path(&full_path).ok_or_else(|| {
            McpError::invalid_params(
                "Unsupported file type".to_string(),
                Some(json!({"path": &request.path})),
            )
        })?;

        let mut parser = FileParser::new(lang_type).map_err(|e| {
            McpError::internal_error(
                format!("Failed to initialize parser: {}", e),
                Some(json!({"language": format!("{:?}", lang_type)})),
            )
        })?;

        let content = if let Some(ref symbol_name) = request.symbol {
            let symbol_content = parser.get_symbol(&source, symbol_name).map_err(|e| {
                McpError::internal_error(
                    format!("Failed to parse file: {}", e),
                    Some(json!({"path": &request.path, "symbol": symbol_name})),
                )
            })?;

            symbol_content.ok_or_else(|| {
                McpError::invalid_params(
                    format!("Symbol '{}' not found", symbol_name),
                    Some(json!({"path": &request.path, "symbol": symbol_name})),
                )
            })?
        } else {
            let symbols = parser.get_outline(&source).map_err(|e| {
                McpError::internal_error(
                    format!("Failed to parse file: {}", e),
                    Some(json!({"path": &request.path})),
                )
            })?;
            Self::format_outline(&symbols)
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
