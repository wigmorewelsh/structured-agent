use crate::cli::args::{AcpArgs, Args, CheckArgs, Command, FileConfig, RunArgs};
use std::env;
use std::fs;
use std::process;

#[derive(Debug, Clone)]
pub struct Config {
    pub program_source: ProgramSource,
    pub mcp_servers: Vec<McpServerConfig>,
    pub engine: EngineType,
    pub with_default_functions: bool,
    pub mode: Mode,
}

#[derive(Debug, Clone)]
pub enum Mode {
    Run,
    Check,
    Acp,
}

#[derive(Debug, Clone)]
pub enum ProgramSource {
    File(String),
    Inline(String),
}

#[derive(Debug, Clone)]
pub enum EngineType {
    Print,
    Gemini,
}

#[derive(Debug, Clone)]
pub struct McpServerConfig {
    pub command: String,
    pub args: Vec<String>,
}

impl Config {
    pub fn from_args(args: Args) -> Self {
        let file_config = args
            .config
            .as_ref()
            .map(|path| Self::load_file_config(path))
            .unwrap_or_default();

        match args.command {
            Command::Run(run_args) => Self::from_run_args(run_args, &file_config),
            Command::Check(check_args) => Self::from_check_args(check_args, &file_config),
            Command::Acp(acp_args) => Self::from_acp_args(acp_args, &file_config),
        }
    }

    fn from_run_args(args: RunArgs, file_config: &FileConfig) -> Self {
        let program_source = Self::merge_program_source(&args.file, &args.inline, file_config);
        let mcp_servers = Self::merge_mcp_servers(&args.mcp_server, file_config);
        let engine = Self::merge_engine(&args.engine, file_config);
        let with_default_functions =
            args.with_default_functions || file_config.with_default_functions.unwrap_or(false);

        Config {
            program_source,
            mcp_servers,
            engine,
            with_default_functions,
            mode: Mode::Run,
        }
    }

    fn from_check_args(args: CheckArgs, file_config: &FileConfig) -> Self {
        let program_source = Self::merge_program_source(&args.file, &args.inline, file_config);
        let mcp_servers = Self::merge_mcp_servers(&args.mcp_server, file_config);
        let with_default_functions =
            args.with_default_functions || file_config.with_default_functions.unwrap_or(false);

        Config {
            program_source,
            mcp_servers,
            engine: EngineType::Print,
            with_default_functions,
            mode: Mode::Check,
        }
    }

    fn from_acp_args(args: AcpArgs, file_config: &FileConfig) -> Self {
        let program_source = Self::merge_program_source(&args.file, &args.inline, file_config);
        let mcp_servers = Self::merge_mcp_servers(&args.mcp_server, file_config);
        let engine = Self::merge_engine(&args.engine, file_config);
        let with_default_functions =
            args.with_default_functions || file_config.with_default_functions.unwrap_or(false);

        Config {
            program_source,
            mcp_servers,
            engine,
            with_default_functions,
            mode: Mode::Acp,
        }
    }

    fn load_file_config(path: &std::path::Path) -> FileConfig {
        let absolute_path = path.canonicalize().unwrap_or_else(|e| {
            eprintln!(
                "Error resolving config file path '{}': {}",
                path.display(),
                e
            );
            process::exit(1);
        });

        if let Some(parent) = absolute_path.parent() {
            if let Err(e) = env::set_current_dir(parent) {
                eprintln!(
                    "Error changing to config directory '{}': {}",
                    parent.display(),
                    e
                );
                process::exit(1);
            }
        }

        let content = fs::read_to_string(&absolute_path).unwrap_or_else(|e| {
            eprintln!(
                "Error reading config file '{}': {}",
                absolute_path.display(),
                e
            );
            process::exit(1);
        });

        toml::from_str(&content).unwrap_or_else(|e| {
            eprintln!(
                "Error parsing config file '{}': {}",
                absolute_path.display(),
                e
            );
            process::exit(1);
        })
    }

    fn merge_program_source(
        file: &Option<String>,
        inline: &Option<String>,
        file_config: &FileConfig,
    ) -> ProgramSource {
        if let Some(inline_code) = inline {
            ProgramSource::Inline(inline_code.clone())
        } else if let Some(file_path) = file {
            ProgramSource::File(file_path.clone())
        } else if let Some(inline_code) = &file_config.inline {
            ProgramSource::Inline(inline_code.clone())
        } else if let Some(file_path) = &file_config.file {
            ProgramSource::File(file_path.clone())
        } else {
            eprintln!("Error: No program specified. Use --file or --inline to provide a program.");
            process::exit(1);
        }
    }

    fn merge_mcp_servers(mcp_server: &[String], file_config: &FileConfig) -> Vec<McpServerConfig> {
        if !mcp_server.is_empty() {
            mcp_server
                .iter()
                .map(|s| Self::parse_mcp_server_config(s))
                .collect()
        } else if let Some(servers) = &file_config.mcp_server {
            servers
                .iter()
                .map(|entry| McpServerConfig {
                    command: entry.command.clone(),
                    args: entry.args.clone(),
                })
                .collect()
        } else {
            vec![]
        }
    }

    fn parse_mcp_server_config(server_spec: &str) -> McpServerConfig {
        let parts: Vec<&str> = server_spec.split_whitespace().collect();
        if parts.is_empty() {
            eprintln!("Error: Empty MCP server specification");
            process::exit(1);
        }

        McpServerConfig {
            command: parts[0].to_string(),
            args: parts[1..].iter().map(|s| s.to_string()).collect(),
        }
    }

    fn merge_engine(engine: &str, file_config: &FileConfig) -> EngineType {
        let engine_str = if engine != "print" {
            engine
        } else if let Some(engine) = &file_config.engine {
            engine
        } else {
            "print"
        };

        match engine_str {
            "gemini" => EngineType::Gemini,
            _ => EngineType::Print,
        }
    }

    pub fn describe_source(&self) -> String {
        match &self.program_source {
            ProgramSource::File(path) => format!("Loading program from: {}", path),
            ProgramSource::Inline(_) => "Executing inline program".to_string(),
        }
    }
}
