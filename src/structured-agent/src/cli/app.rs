use crate::acp;
use crate::cli::config::Config;
use crate::cli::errors::CliError;
use crate::runtime::{Runtime, load_program};

pub struct App;

impl App {
    pub async fn run(config: Config) -> Result<(), CliError> {
        if config.acp_mode {
            return Self::run_acp_mode(config).await;
        }

        println!("{}", config.describe_source());

        let program = load_program(&config.program_source).map_err(CliError::from)?;

        if !config.mcp_servers.is_empty() {
            println!("MCP servers configured: {}", config.mcp_servers.len());
            for server in &config.mcp_servers {
                println!("  - {} {}", server.command, server.args.join(" "));
            }
        }

        println!("Initializing structured agent runtime...");

        let runtime = Runtime::builder(program.clone())
            .from_config(&config)
            .await
            .map_err(CliError::RuntimeError)?;

        if config.check_only {
            println!("Running checks...");
            match runtime.check() {
                Ok(_) => {
                    println!("All checks passed");
                    Ok(())
                }
                Err(e) => Err(CliError::RuntimeError(format!("{}", e))),
            }
        } else {
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
    }

    async fn run_acp_mode(config: Config) -> Result<(), CliError> {
        acp::run_acp_server(config)
            .await
            .map_err(|e| CliError::RuntimeError(format!("ACP server error: {}", e)))
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
            crate::runtime::ExprResult::List(list) => {
                use arrow::array::Array;
                println!("Result: List[{}]", list.len());
            }
            crate::runtime::ExprResult::Option(opt) => match opt {
                Some(inner) => {
                    print!("Result: Some(");
                    Self::display_result(inner);
                    println!(")");
                }
                None => {
                    println!("Result: None");
                }
            },
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
            acp_mode: false,
            check_only: false,
        };

        let program = load_program(&config.program_source).unwrap();
        let runtime = Runtime::builder(program)
            .from_config(&config)
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
            acp_mode: false,
            check_only: false,
        };

        let program = load_program(&config.program_source).unwrap();
        let runtime = Runtime::builder(program)
            .from_config(&config)
            .await
            .unwrap();

        let functions = runtime.list_functions();
        assert!(!functions.contains(&"input"));
        assert!(!functions.contains(&"print"));
    }
}
