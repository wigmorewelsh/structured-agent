mod acp;
mod analysis;
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

use clap::Parser;
use cli::{App, Args, Config};
use std::process;
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let args = Args::parse();
    let config = Config::from_args(args);

    if let Err(e) = App::run(config).await {
        eprintln!("Error: {}", e);
        process::exit(1);
    }
}
