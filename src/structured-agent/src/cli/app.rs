use crate::cli::config::{Config, EngineType, ProgramSource};
use crate::cli::errors::CliError;
use crate::functions::{InputFunction, PrintFunction};
use crate::gemini::GeminiEngine;
use crate::mcp::McpClient;
use crate::runtime::Runtime;
use crate::types::LanguageEngine;
use std::fs;
use std::rc::Rc;
use std::sync::Arc;

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
        let runtime =
            Self::build_runtime(mcp_clients, &config.engine, config.with_default_functions).await?;

        println!("Executing program...");

        match runtime.run(&program).await {
            Ok(result) => {
                println!("Program executed successfully");
                Self::display_result(&result);
                Ok(())
            }
            Err(e) => Err(CliError::RuntimeError(format!("{}", e))),
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

    async fn build_runtime(
        mcp_clients: Vec<McpClient>,
        engine_type: &EngineType,
        with_default_functions: bool,
    ) -> Result<Runtime, CliError> {
        let mut runtime_builder = Runtime::builder();

        for client in mcp_clients {
            runtime_builder = runtime_builder.with_mcp_client(client);
        }

        let engine: Rc<dyn LanguageEngine> = match engine_type {
            EngineType::Print => Rc::new(crate::types::PrintEngine {}),
            EngineType::Gemini => match GeminiEngine::from_env().await {
                Ok(gemini) => Rc::new(gemini),
                Err(e) => {
                    return Err(CliError::RuntimeError(format!(
                        "Failed to initialize Gemini engine: {}. Make sure you're authenticated with 'gcloud auth application-default login'",
                        e
                    )));
                }
            },
        };

        runtime_builder = runtime_builder.with_engine(engine);

        if with_default_functions {
            runtime_builder = runtime_builder
                .with_native_function(Arc::new(InputFunction::new()))
                .with_native_function(Arc::new(PrintFunction::new()));
        }

        Ok(runtime_builder.build())
    }

    fn display_result(result: &crate::runtime::ExprResult) {
        match result {
            crate::runtime::ExprResult::String(s) => {
                println!("\n═══ Agent Response ═══");

                let cleaned = s.trim();

                if cleaned.contains('\n') {
                    let mut in_code_block = false;

                    for line in cleaned.lines() {
                        let trimmed_line = line.trim();

                        if trimmed_line.starts_with("```") {
                            in_code_block = !in_code_block;
                            if in_code_block {
                                println!("\n┌─ Code Block ─");
                            } else {
                                println!("└─────────────");
                            }
                            continue;
                        }

                        if in_code_block {
                            println!("│ {}", line);
                        } else if trimmed_line.is_empty() {
                            println!();
                        } else {
                            println!("{}", line);
                        }
                    }
                } else {
                    println!("{}", cleaned);
                }

                println!("═══════════════════════");
            }
            crate::runtime::ExprResult::Unit => {
                println!("Result: (no output)");
            }
            crate::runtime::ExprResult::Boolean(b) => {
                println!("Result: {}", b);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::config::EngineType;

    #[tokio::test]
    async fn test_build_runtime_with_default_functions() {
        let runtime = App::build_runtime(vec![], &EngineType::Print, true)
            .await
            .unwrap();

        let functions = runtime.list_functions();
        assert!(functions.contains(&"input"));
        assert!(functions.contains(&"print"));
    }

    #[tokio::test]
    async fn test_build_runtime_without_default_functions() {
        let runtime = App::build_runtime(vec![], &EngineType::Print, false)
            .await
            .unwrap();

        let functions = runtime.list_functions();
        assert!(!functions.contains(&"input"));
        assert!(!functions.contains(&"print"));
    }
}
