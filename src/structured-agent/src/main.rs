use clap::Parser;
use opentelemetry::trace::TracerProvider as OtelTracerProvider;
use opentelemetry_otlp::WithExportConfig;
use std::process;
use structured_agent::cli::{App, Args, Config};
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

struct TelemetryGuard;

impl Drop for TelemetryGuard {
    fn drop(&mut self) {
        opentelemetry::global::shutdown_tracer_provider();
    }
}

fn main() {
    let args = Args::parse();
    let config = Config::from_args(args);

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("failed to build tokio runtime");

    let exit_code = rt.block_on(async_main(config));
    process::exit(exit_code);
}

async fn async_main(config: Config) -> i32 {
    let _telemetry = init_telemetry(&config).await;
    log::set_max_level(log_max_level_from_env());

    match App::run(config).await {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("Error: {}", e);
            1
        }
    }
}

async fn init_telemetry(config: &Config) -> TelemetryGuard {
    let obs = &config.observability;

    let mut loki_task: Option<tracing_loki::BackgroundTask> = None;
    let loki_layer = obs.loki_url.as_ref().and_then(|url_str| {
        let url = url::Url::parse(url_str).ok()?;
        let (layer, task) = tracing_loki::builder()
            .label("app", "structured-agent")
            .ok()?
            .build_url(url)
            .ok()?;
        loki_task = Some(task);
        Some(layer)
    });

    let otel_layer = obs.otlp_endpoint.as_ref().and_then(|endpoint| {
        let provider = opentelemetry_otlp::new_pipeline()
            .tracing()
            .with_exporter(
                opentelemetry_otlp::new_exporter()
                    .tonic()
                    .with_endpoint(endpoint),
            )
            .with_trace_config(opentelemetry_sdk::trace::Config::default().with_resource(
                opentelemetry_sdk::Resource::new(vec![opentelemetry::KeyValue::new(
                    "service.name",
                    "structured-agent",
                )]),
            ))
            .install_batch(opentelemetry_sdk::runtime::Tokio)
            .map_err(|e| eprintln!("Warning: failed to init OTLP tracer: {}", e))
            .ok()?;
        let tracer = provider.tracer("structured-agent");
        Some(tracing_opentelemetry::layer().with_tracer(tracer))
    });

    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .with(tracing_subscriber::fmt::layer().with_writer(std::io::stderr))
        .with(otel_layer)
        .with(loki_layer)
        .init();

    if let Some(task) = loki_task {
        tokio::spawn(task);
    }

    if let Some(port) = obs.metrics_port {
        if let Err(e) = metrics_exporter_prometheus::PrometheusBuilder::new()
            .with_http_listener(([0, 0, 0, 0], port))
            .install()
        {
            eprintln!(
                "Warning: failed to start metrics exporter on :{}: {}",
                port, e
            );
        }
    }

    TelemetryGuard
}

fn log_max_level_from_env() -> log::LevelFilter {
    std::env::var("RUST_LOG")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(log::LevelFilter::Info)
}
