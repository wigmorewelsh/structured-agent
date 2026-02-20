use super::test_helpers::{TestAgent, run_local};
use std::fs;
use std::io::Write;
use tempfile::NamedTempFile;

#[tokio::test]
async fn test_reload_executes_new_code() {
    run_local(|| async {
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "extern fn print(value: String): ()").unwrap();
        writeln!(temp_file, "fn main() {{ print(\"original\") }}").unwrap();
        temp_file.flush().unwrap();

        let file_path = temp_file.path().to_str().unwrap().to_string();

        let config = structured_agent::cli::config::Config {
            program_source: structured_agent::cli::config::ProgramSource::File(file_path.clone()),
            engine: structured_agent::cli::config::EngineType::Print,
            mcp_servers: vec![],
            with_default_functions: true,
            with_unstable_functions: false,
            mode: structured_agent::cli::config::Mode::Acp,
        };

        let mut agent = TestAgent::from_config(config).await;

        fs::write(
            temp_file.path(),
            "extern fn print(value: String): ()\nfn main() { print(\"reloaded\") }",
        )
        .unwrap();

        agent.reload().await;
    })
    .await;
}
