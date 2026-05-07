use crate::acp;
use crate::cli::config::{Config, Mode};
use crate::cli::errors::CliError;
use crate::cli::user::User;
use crate::runtime::Runtime;
use std::sync::Arc;

pub struct App;

impl App {
    pub async fn run(config: Config) -> Result<(), CliError> {
        match config.mode {
            Mode::Acp => Self::run_acp_mode(config).await,
            Mode::Check => Self::run_check_mode(config).await,
            Mode::Run => Self::run_execute_mode(config).await,
            Mode::DumpIl => Self::run_dump_il_mode(config).await,
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
        let user = User::new(Arc::new(runtime));
        match user.run().await {
            Ok(result) => {
                println!("Program executed successfully");
                Self::display_result(&result);
                Ok(())
            }
            Err(e) => Err(CliError::Runtime(format!("{}", e))),
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
            Err(e) => Err(CliError::Runtime(format!("{}", e))),
        }
    }

    async fn build_runtime(config: &Config) -> Result<Runtime, CliError> {
        let mut builder = Runtime::builder(config.program_source.clone());
        if let Ok(cwd) = std::env::current_dir() {
            builder = builder.with_mcp_working_dir(cwd.to_string_lossy().into_owned());
        }
        builder.with_config(config).await.map_err(CliError::Runtime)
    }

    async fn run_dump_il_mode(config: Config) -> Result<(), CliError> {
        let runtime = Self::build_runtime(&config).await?;
        match runtime.dump_il() {
            Ok(functions) => {
                for func in functions {
                    print!("{}", func);
                }
                Ok(())
            }
            Err(e) => Err(CliError::Runtime(format!("{}", e))),
        }
    }

    async fn run_acp_mode(config: Config) -> Result<(), CliError> {
        acp::run_acp_server(config)
            .await
            .map_err(|e| CliError::Runtime(format!("ACP server error: {}", e)))
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
    async fn test_build_runtime_runs_successfully() {
        let config = Config {
            program_source: crate::cli::config::ProgramSource::Inline(
                "use io::print\n\nfn main(): () { print(\"hello\") }".to_string(),
            ),
            mcp_servers: vec![],
            engine: EngineType::Print,
            with_unstable_functions: false,
            mode: Mode::Run,
        };

        let runtime = Runtime::builder(config.program_source.clone())
            .with_config(&config)
            .await
            .unwrap();

        let result = runtime.run().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_dump_il_returns_compiled_functions() {
        let config = Config {
            program_source: crate::cli::config::ProgramSource::Inline(
                "fn add(a: Int, b: Int): Int { a }

fn main(): () { add(1, 2) }"
                    .to_string(),
            ),
            mcp_servers: vec![],
            engine: EngineType::Print,
            with_unstable_functions: false,
            mode: Mode::DumpIl,
        };

        let runtime = Runtime::builder(config.program_source.clone())
            .with_config(&config)
            .await
            .unwrap();

        let functions = runtime.dump_il().unwrap();
        let names: Vec<String> = functions.iter().map(|f| f.name.to_string()).collect();
        assert!(names.iter().any(|n| n.contains("main")));
        assert!(names.iter().any(|n| n.contains("add")));
    }
}
