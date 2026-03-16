use crate::acp;
use crate::cli::config::{Config, Mode};
use crate::cli::errors::CliError;
use crate::runtime::Runtime;

pub struct App;

impl App {
    pub async fn run(config: Config) -> Result<(), CliError> {
        match config.mode {
            Mode::Acp => Self::run_acp_mode(config).await,
            Mode::Check => Self::run_check_mode(config).await,
            Mode::Run => Self::run_execute_mode(config).await,
        }
    }

    async fn run_execute_mode(config: Config) -> Result<(), CliError> {
        println!("{}", config.describe_source());

        if !config.mcp_servers.is_empty() {
            println!("MCP servers configured: {}", config.mcp_servers.len());
            for server in &config.mcp_servers {
                println!("  - {} {}", server.command, server.args.join(" "));
            }
        }

        println!("Initializing structured agent runtime...");

        let runtime = Self::build_runtime(&config).await?;

        println!("Executing program...");
        match runtime.run().await {
            Ok(result) => {
                println!("Program executed successfully");
                Self::display_result(&result);
                Ok(())
            }
            Err(e) => Err(CliError::RuntimeError(format!("{}", e))),
        }
    }

    async fn run_check_mode(config: Config) -> Result<(), CliError> {
        println!("{}", config.describe_source());

        if !config.mcp_servers.is_empty() {
            println!("MCP servers configured: {}", config.mcp_servers.len());
            for server in &config.mcp_servers {
                println!("  - {} {}", server.command, server.args.join(" "));
            }
        }

        println!("Initializing structured agent runtime...");

        let runtime = Self::build_runtime(&config).await?;

        println!("Running checks...");
        match runtime.check() {
            Ok(_) => {
                println!("All checks passed");
                Ok(())
            }
            Err(e) => Err(CliError::RuntimeError(format!("{}", e))),
        }
    }

    async fn build_runtime(config: &Config) -> Result<Runtime, CliError> {
        Runtime::builder(config.program_source.clone())
            .with_config(config)
            .await
            .map_err(CliError::RuntimeError)
    }

    async fn run_acp_mode(config: Config) -> Result<(), CliError> {
        acp::run_acp_server(config)
            .await
            .map_err(|e| CliError::RuntimeError(format!("ACP server error: {}", e)))
    }

    fn display_result(result: &crate::runtime::ExpressionValue) {
        if let Ok(s) = result.as_string() {
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
        } else if result.type_name() == "Unit" {
            println!("Result: (no output)");
        } else if let Ok(b) = result.as_boolean() {
            println!("Result: {}", b);
        } else if result.as_list().is_ok() {
            println!("Result: {}", result.format_for_llm());
        } else {
            println!("Result: {}", result.value_string());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::config::EngineType;

    #[tokio::test]
    async fn test_build_runtime_with_default_functions() {
        let config = Config {
            program_source: crate::cli::config::ProgramSource::Inline(
                "fn main(): () {}".to_string(),
            ),
            mcp_servers: vec![],
            engine: EngineType::Print,
            with_default_functions: true,
            with_unstable_functions: false,
            with_acp_functions: false,
            mode: Mode::Run,
        };

        let runtime = Runtime::builder(config.program_source.clone())
            .with_config(&config)
            .await
            .unwrap();

        let functions = runtime.list_functions();
        assert!(functions.contains(&"input"));
        assert!(functions.contains(&"print"));
    }

    #[tokio::test]
    async fn test_build_runtime_without_default_functions() {
        let config = Config {
            program_source: crate::cli::config::ProgramSource::Inline(
                "fn main(): () {}".to_string(),
            ),
            mcp_servers: vec![],
            engine: EngineType::Print,
            with_default_functions: false,
            with_unstable_functions: false,
            with_acp_functions: false,
            mode: Mode::Run,
        };

        let runtime = Runtime::builder(config.program_source.clone())
            .with_config(&config)
            .await
            .unwrap();

        let functions = runtime.list_functions();
        assert!(!functions.contains(&"input"));
        assert!(!functions.contains(&"print"));
    }
}
