mod directory;
mod file;

use directory::WorkspaceDirectory;
use file::WorkspaceFile;
use rmcp::service::{NotificationContext, Peer, RoleServer};
use rmcp::{
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::*,
    schemars, tool, tool_handler, tool_router, ErrorData as McpError, ServerHandler,
};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct ListDirectoryRequest {
    #[schemars(
        description = "Path to the directory relative to the workspace root. Use '.' for the root."
    )]
    pub path: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct ReadFileRequest {
    #[schemars(description = "Path to the file relative to the workspace root")]
    pub path: String,
    #[schemars(description = "Optional symbol name to retrieve its full definition")]
    pub symbol: Option<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct ReplaceSymbolRequest {
    #[schemars(description = "Path to the file relative to the workspace root")]
    pub path: String,
    #[schemars(description = "Name of the symbol to replace")]
    pub symbol: String,
    #[schemars(description = "Full replacement text for the symbol")]
    pub replacement: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct WriteFileRequest {
    #[schemars(description = "Path to the file relative to the workspace root")]
    pub path: String,
    #[schemars(description = "Content to write to the file")]
    pub content: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct PatchLinesRequest {
    #[schemars(description = "Path to the file relative to the workspace root")]
    pub path: String,
    #[schemars(description = "Hash anchor of the first line in the range to replace")]
    pub start_anchor: String,
    #[schemars(description = "Hash anchor of the last line in the range to replace")]
    pub end_anchor: String,
    #[schemars(description = "Replacement text (plain, without anchor prefixes)")]
    pub replacement: String,
}

#[derive(Clone)]
pub struct WorkspaceServer {
    workspace_root: Arc<RwLock<Option<PathBuf>>>,
    roots_generation: Arc<AtomicU64>,
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl WorkspaceServer {
    pub fn new(workspace_root: PathBuf) -> Self {
        Self {
            workspace_root: Arc::new(RwLock::new(Some(workspace_root))),
            roots_generation: Arc::new(AtomicU64::new(0)),
            tool_router: Self::tool_router(),
        }
    }

    pub fn new_uninitialized() -> Self {
        Self {
            workspace_root: Arc::new(RwLock::new(None)),
            roots_generation: Arc::new(AtomicU64::new(0)),
            tool_router: Self::tool_router(),
        }
    }

    fn workspace_root(&self) -> Result<PathBuf, McpError> {
        self.workspace_root
            .read()
            .unwrap()
            .clone()
            .ok_or_else(|| McpError::invalid_request("Workspace root not set", None))
    }

    async fn apply_roots(&self, peer: &Peer<RoleServer>) {
        let generation = self.roots_generation.fetch_add(1, Ordering::SeqCst) + 1;
        match peer.list_roots().await {
            Ok(result) => {
                if self.roots_generation.load(Ordering::SeqCst) != generation {
                    return;
                }
                if let Some(root) = result.roots.first() {
                    let path = PathBuf::from(root.uri.trim_start_matches("file://"));
                    match path.canonicalize() {
                        Ok(canonical) => {
                            tracing::info!("Workspace root set to: {:?}", canonical);
                            *self.workspace_root.write().unwrap() = Some(canonical);
                        }
                        Err(e) => tracing::warn!("Cannot canonicalize workspace root: {e}"),
                    }
                }
            }
            Err(e) => tracing::warn!("Failed to list roots: {e:?}"),
        }
    }

    #[tool(description = "List files and directories in a workspace directory.")]
    pub fn list_directory(
        &self,
        Parameters(request): Parameters<ListDirectoryRequest>,
    ) -> Result<CallToolResult, McpError> {
        let root = self.workspace_root()?;
        let dir = WorkspaceDirectory::open(&root, &request.path)?;
        Ok(CallToolResult::success(vec![Content::text(dir.list())]))
    }

    #[tool(
        description = "Read a file from the workspace. Returns a tree-sitter outline showing top-level symbols (functions, types, structs, etc.) with their line numbers. If a symbol name is provided, returns the full definition of that symbol instead."
    )]
    pub fn read_file(
        &self,
        Parameters(request): Parameters<ReadFileRequest>,
    ) -> Result<CallToolResult, McpError> {
        let root = self.workspace_root()?;
        let file = WorkspaceFile::open(&root, &request.path)?;

        let content = match &request.symbol {
            Some(symbol_name) => file.get_symbol(symbol_name)?,
            None => file.get_outline()?,
        };

        Ok(CallToolResult::success(vec![Content::text(content)]))
    }

    #[tool(
        description = "Replace the entire contents of a file. Creates the file if it does not exist."
    )]
    pub fn write_file(
        &self,
        Parameters(request): Parameters<WriteFileRequest>,
    ) -> Result<CallToolResult, McpError> {
        let root = self.workspace_root()?;
        WorkspaceFile::write_file(&root, &request.path, &request.content)?;
        Ok(CallToolResult::success(vec![Content::text("File written")]))
    }

    #[tool(
        description = "Replace a named symbol in a file. The symbol is located using tree-sitter and its exact source range is replaced."
    )]
    pub fn replace_symbol(
        &self,
        Parameters(request): Parameters<ReplaceSymbolRequest>,
    ) -> Result<CallToolResult, McpError> {
        let root = self.workspace_root()?;
        WorkspaceFile::replace_symbol(&root, &request.path, &request.symbol, &request.replacement)?;
        Ok(CallToolResult::success(vec![Content::text(
            "Symbol replaced",
        )]))
    }

    #[tool(
        description = "Replace a contiguous range of lines identified by hash anchors. Read the file first to obtain anchors from the annotated output."
    )]
    pub fn patch_lines(
        &self,
        Parameters(request): Parameters<PatchLinesRequest>,
    ) -> Result<CallToolResult, McpError> {
        let root = self.workspace_root()?;
        WorkspaceFile::patch_lines_with_anchors(
            &root,
            &request.path,
            &request.start_anchor,
            &request.end_anchor,
            &request.replacement,
        )?;
        Ok(CallToolResult::success(vec![Content::text(
            "Lines patched",
        )]))
    }
}

#[tool_handler]
impl ServerHandler for WorkspaceServer {
    async fn on_initialized(&self, context: NotificationContext<RoleServer>) {
        let this = self.clone();
        let peer = context.peer.clone();
        tokio::spawn(async move {
            this.apply_roots(&peer).await;
        });
    }

    async fn on_roots_list_changed(&self, context: NotificationContext<RoleServer>) {
        let this = self.clone();
        let peer = context.peer.clone();
        tokio::spawn(async move {
            this.apply_roots(&peer).await;
        });
    }

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
