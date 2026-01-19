use crate::cli::config::{Config, ProgramSource};
use crate::cli::errors::CliError;
use crate::mcp::McpClient;
use crate::runtime::Runtime;
use std::fs;

pub struct App;

impl App {
    pub async fn run(config: Config) -> Result<(), CliError> {
        println!("{}", config.describe_source());

        let program = Self::load_program(&config.program_source)?;

        if !config.mcp_servers.is_empty() {
            println!("MCP servers configured: {}", config.mcp_servers.len());
            for server in &config.mcp_servers {
                println!("  - {} {}", server.command, server.args.join(" "));
            }
        }

        println!("Initializing structured agent runtime...");

        let mcp_clients = Self::create_mcp_clients(&config.mcp_servers).await?;
        let runtime = Self::build_runtime(mcp_clients);

        println!("Executing program...");

        match runtime.run(&program).await {
            Ok(result) => {
                println!("Program executed successfully");
                println!("Result: {:?}", result);
                Ok(())
            }
            Err(e) => Err(CliError::RuntimeError(format!("{:?}", e))),
        }
    }

    fn load_program(source: &ProgramSource) -> Result<String, CliError> {
        match source {
            ProgramSource::Inline(code) => Ok(code.clone()),
            ProgramSource::File(path) => fs::read_to_string(path).map_err(CliError::from),
        }
    }

    async fn create_mcp_clients(
        server_configs: &[crate::cli::config::McpServerConfig],
    ) -> Result<Vec<McpClient>, CliError> {
        let mut clients = Vec::new();

        for config in server_configs {
            match McpClient::new_stdio(&config.command, config.args.clone()).await {
                Ok(client) => {
                    println!("Connected to MCP server: {}", config.command);
                    clients.push(client);
                }
                Err(e) => {
                    return Err(CliError::McpError(format!(
                        "Failed to connect to MCP server '{}': {}",
                        config.command, e
                    )));
                }
            }
        }

        Ok(clients)
    }

    fn build_runtime(mcp_clients: Vec<McpClient>) -> Runtime {
        let mut runtime_builder = Runtime::builder();

        for client in mcp_clients {
            runtime_builder = runtime_builder.with_mcp_client(client);
        }

        runtime_builder.build()
    }
}
