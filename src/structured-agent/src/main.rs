use clap::Parser;
use std::process;
use structured_agent::cli::{App, Args, Config};
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
