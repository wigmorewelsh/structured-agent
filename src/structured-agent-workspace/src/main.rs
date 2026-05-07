use anyhow::Result;
use clap::Parser;
use rmcp::{transport::stdio, ServiceExt};
use tracing_subscriber::{self, EnvFilter};

mod cli;
pub mod parser;
pub mod workspace;

use cli::Args;
use workspace::WorkspaceServer;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive(tracing::Level::INFO.into()))
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .init();

    let args = Args::parse();

    let workspace_root = args.workspace.canonicalize()?;

    if !workspace_root.is_dir() {
        anyhow::bail!("Workspace path is not a directory: {:?}", workspace_root);
    }

    tracing::info!("Starting structured-agent-workspace MCP server");
    tracing::info!("Workspace root: {:?}", workspace_root);

    let service = WorkspaceServer::new(workspace_root)
        .serve(stdio())
        .await
        .inspect_err(|e| {
            tracing::error!("serving error: {:?}", e);
        })?;

    service.waiting().await?;
    Ok(())
}
