use clap::{Parser, Subcommand};
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "structured-agent")]
#[command(version = "0.1.0")]
#[command(about = "A structured agent runtime with MCP support")]
pub struct Args {
    #[command(subcommand)]
    pub command: Command,

    #[arg(
        short = 'c',
        long,
        global = true,
        value_name = "FILE",
        help = "Path to configuration file (TOML format)"
    )]
    pub config: Option<PathBuf>,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    #[command(about = "Run a program")]
    Run(RunArgs),

    #[command(about = "Parse and typecheck a program without executing")]
    Check(CheckArgs),

    #[command(about = "Run as ACP (Agent Client Protocol) server")]
    Acp(AcpArgs),

    #[command(about = "Dump the IL bytecode for a program")]
    DumpIl(DumpIlArgs),
}

#[derive(Parser, Debug)]
pub struct RunArgs {
    #[arg(short = 'f', long, value_name = "FILE", conflicts_with = "inline")]
    pub file: Option<String>,

    #[arg(short = 'i', long, value_name = "CODE", conflicts_with = "file")]
    pub inline: Option<String>,

    #[arg(
        short = 'm',
        long,
        value_name = "COMMAND",
        help = "MCP server command (format: 'command arg1 arg2')"
    )]
    pub mcp_server: Vec<String>,

    #[arg(
        short = 'e',
        long,
        value_name = "ENGINE",
        default_value = "print",
        help = "Language engine to use: 'print' for console output, 'gemini' for AI responses"
    )]
    pub engine: String,

    #[arg(
        long,
        help = "Include unstable functions (head, tail, is_some, some_value, is_some_list, some_value_list)"
    )]
    pub with_unstable_functions: bool,

    #[arg(long, value_name = "KEY", help = "Gemini API key for authentication")]
    pub gemini_api_key: Option<String>,

    #[arg(
        long,
        value_name = "MODEL",
        help = "Gemini model to use: gemini-2.5-pro, gemini-2.5-flash, gemini-2.5-flash-lite, gemini-3-flash-preview, gemini-3-pro-preview, or custom model name"
    )]
    pub gemini_model: Option<String>,

    #[arg(long, value_name = "KEY", help = "OpenAI API key")]
    pub openai_api_key: Option<String>,

    #[arg(long, value_name = "MODEL", help = "OpenAI model name")]
    pub openai_model: Option<String>,

    #[arg(long, value_name = "URL", help = "OpenAI-compatible base URL")]
    pub openai_base_url: Option<String>,

    #[arg(long, value_name = "TOKEN", help = "Hugging Face API token")]
    pub hf_token: Option<String>,

    #[arg(long, value_name = "MODEL", help = "Hugging Face model name")]
    pub hf_model: Option<String>,

    #[arg(
        long,
        value_name = "TOKEN",
        help = "GitHub token for Copilot authentication"
    )]
    pub github_token: Option<String>,

    #[arg(long, value_name = "MODEL", help = "Copilot model name")]
    pub copilot_model: Option<String>,

    #[arg(
        long,
        value_name = "PORT",
        help = "Expose Prometheus metrics scrape endpoint on this port"
    )]
    pub metrics_port: Option<u16>,

    #[arg(
        long,
        value_name = "URL",
        help = "OTLP gRPC endpoint for trace export (e.g. http://localhost:4317)"
    )]
    pub otlp_endpoint: Option<String>,

    #[arg(
        long,
        value_name = "URL",
        help = "Loki push URL for log shipping (e.g. http://localhost:3100)"
    )]
    pub loki_url: Option<String>,
}

#[derive(Parser, Debug)]
pub struct CheckArgs {
    #[arg(short = 'f', long, value_name = "FILE", conflicts_with = "inline")]
    pub file: Option<String>,

    #[arg(short = 'i', long, value_name = "CODE", conflicts_with = "file")]
    pub inline: Option<String>,

    #[arg(
        short = 'm',
        long,
        value_name = "COMMAND",
        help = "MCP server command (format: 'command arg1 arg2')"
    )]
    pub mcp_server: Vec<String>,

    #[arg(
        long,
        help = "Include unstable functions (head, tail, is_some, some_value, is_some_list, some_value_list)"
    )]
    pub with_unstable_functions: bool,
}

#[derive(Parser, Debug)]
pub struct AcpArgs {
    #[arg(short = 'f', long, value_name = "FILE", conflicts_with = "inline")]
    pub file: Option<String>,

    #[arg(short = 'i', long, value_name = "CODE", conflicts_with = "file")]
    pub inline: Option<String>,

    #[arg(
        short = 'm',
        long,
        value_name = "COMMAND",
        help = "MCP server command (format: 'command arg1 arg2')"
    )]
    pub mcp_server: Vec<String>,

    #[arg(
        short = 'e',
        long,
        value_name = "ENGINE",
        default_value = "print",
        help = "Language engine to use: 'print' for console output, 'gemini' for AI responses"
    )]
    pub engine: String,

    #[arg(
        long,
        help = "Include unstable functions (head, tail, is_some, some_value, is_some_list, some_value_list)"
    )]
    pub with_unstable_functions: bool,

    #[arg(long, value_name = "KEY", help = "Gemini API key for authentication")]
    pub gemini_api_key: Option<String>,

    #[arg(
        long,
        value_name = "MODEL",
        help = "Gemini model to use: gemini-2.5-pro, gemini-2.5-flash, gemini-2.5-flash-lite, gemini-3-flash-preview, gemini-3-pro-preview, or custom model name"
    )]
    pub gemini_model: Option<String>,

    #[arg(long, value_name = "KEY", help = "OpenAI API key")]
    pub openai_api_key: Option<String>,

    #[arg(long, value_name = "MODEL", help = "OpenAI model name")]
    pub openai_model: Option<String>,

    #[arg(long, value_name = "URL", help = "OpenAI-compatible base URL")]
    pub openai_base_url: Option<String>,

    #[arg(long, value_name = "TOKEN", help = "Hugging Face API token")]
    pub hf_token: Option<String>,

    #[arg(long, value_name = "MODEL", help = "Hugging Face model name")]
    pub hf_model: Option<String>,

    #[arg(
        long,
        value_name = "TOKEN",
        help = "GitHub token for Copilot authentication"
    )]
    pub github_token: Option<String>,

    #[arg(long, value_name = "MODEL", help = "Copilot model name")]
    pub copilot_model: Option<String>,

    #[arg(
        long,
        value_name = "PORT",
        help = "Expose Prometheus metrics scrape endpoint on this port"
    )]
    pub metrics_port: Option<u16>,

    #[arg(
        long,
        value_name = "URL",
        help = "OTLP gRPC endpoint for trace export (e.g. http://localhost:4317)"
    )]
    pub otlp_endpoint: Option<String>,

    #[arg(
        long,
        value_name = "URL",
        help = "Loki push URL for log shipping (e.g. http://localhost:3100)"
    )]
    pub loki_url: Option<String>,
}

#[derive(Parser, Debug)]
pub struct DumpIlArgs {
    #[arg(short = 'f', long, value_name = "FILE", conflicts_with = "inline")]
    pub file: Option<String>,

    #[arg(short = 'i', long, value_name = "CODE", conflicts_with = "file")]
    pub inline: Option<String>,

    #[arg(
        long,
        help = "Include unstable functions (head, tail, is_some, some_value, is_some_list, some_value_list)"
    )]
    pub with_unstable_functions: bool,
}

#[derive(Deserialize, Debug, Default)]
pub struct FileConfig {
    pub file: Option<String>,
    pub inline: Option<String>,
    pub mcp_server: Option<Vec<McpServerEntry>>,
    pub engine: Option<String>,
    pub with_unstable_functions: Option<bool>,
    pub gemini_api_key: Option<String>,
    pub gemini_model: Option<String>,
    pub openai_api_key: Option<String>,
    pub openai_model: Option<String>,
    pub openai_base_url: Option<String>,
    pub hf_token: Option<String>,
    pub hf_model: Option<String>,
    pub github_token: Option<String>,
    pub copilot_model: Option<String>,
    pub metrics_port: Option<u16>,
    pub otlp_endpoint: Option<String>,
    pub loki_url: Option<String>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct McpServerEntry {
    pub command: String,
    pub args: Vec<String>,
    pub working_dir: Option<String>,
}
