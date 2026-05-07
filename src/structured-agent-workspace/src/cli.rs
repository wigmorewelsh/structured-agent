use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "structured-agent-workspace")]
#[command(version = "0.1.0")]
#[command(about = "MCP server for structured file reading with tree-sitter")]
pub struct Args {
    #[arg(
        short = 'w',
        long,
        value_name = "DIR",
        help = "Workspace root directory"
    )]
    pub workspace: PathBuf,
}
