use clap::ArgMatches;
use std::process;

#[derive(Debug)]
pub struct Config {
    pub program_source: ProgramSource,
    pub mcp_servers: Vec<McpServerConfig>,
    pub engine: EngineType,
    pub with_default_functions: bool,
}

#[derive(Debug)]
pub enum ProgramSource {
    File(String),
    Inline(String),
}

#[derive(Debug)]
pub enum EngineType {
    Print,
    Gemini,
}

#[derive(Debug)]
pub struct McpServerConfig {
    pub command: String,
    pub args: Vec<String>,
}

impl Config {
    pub fn from_matches(matches: &ArgMatches) -> Self {
        let program_source = Self::parse_program_source(matches);
        let mcp_servers = Self::parse_mcp_servers(matches);
        let engine = Self::parse_engine(matches);
        let with_default_functions = matches.get_flag("with-default-functions");

        Config {
            program_source,
            mcp_servers,
            engine,
            with_default_functions,
        }
    }

    fn parse_program_source(matches: &ArgMatches) -> ProgramSource {
        if let Some(inline_code) = matches.get_one::<String>("inline") {
            ProgramSource::Inline(inline_code.clone())
        } else if let Some(file_path) = matches.get_one::<String>("file") {
            ProgramSource::File(file_path.clone())
        } else {
            eprintln!("Error: No program specified. Use --file or --inline to provide a program.");
            process::exit(1);
        }
    }

    fn parse_mcp_servers(matches: &ArgMatches) -> Vec<McpServerConfig> {
        matches
            .get_many::<String>("mcp-server")
            .map(|servers| servers.map(|s| Self::parse_mcp_server_config(s)).collect())
            .unwrap_or_default()
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

    fn parse_engine(matches: &ArgMatches) -> EngineType {
        match matches
            .get_one::<String>("engine")
            .map(|s| s.as_str())
            .unwrap_or("print")
        {
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
