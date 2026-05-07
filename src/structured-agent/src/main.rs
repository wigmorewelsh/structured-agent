use clap::Parser;
use std::process;
use structured_agent::cli::{App, Args, Config};
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

fn log_max_level_from_env() -> log::LevelFilter {
    std::env::var("RUST_LOG")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(log::LevelFilter::Info)
}

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .with(tracing_subscriber::fmt::layer().with_writer(std::io::stderr))
        .init();
    log::set_max_level(log_max_level_from_env());

    let args = Args::parse();
    let config = Config::from_args(args);

    if let Err(e) = App::run(config).await {
        eprintln!("Error: {}", e);
        process::exit(1);
    }
}
