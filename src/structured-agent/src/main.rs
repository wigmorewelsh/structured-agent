mod ast;
mod cli;
mod compiler;
mod diagnostics;
mod expressions;
mod functions;
mod gemini;
mod mcp;
mod runtime;
mod typecheck;
mod types;

use cli::{App, Config, build_cli};
use std::process;

#[tokio::main]
async fn main() {
    let matches = build_cli().get_matches();
    let config = Config::from_matches(&matches);

    if let Err(e) = App::run(config).await {
        eprintln!("Error: {}", e);
        process::exit(1);
    }
}
