# Observability Stack

## Goal

Add metrics, tracing, and log aggregation to the structured-agent runtime. The work covered three areas: instrumenting the Gemini engine with metrics, wiring telemetry initialisation into the binary, and standing up a local observability stack (Prometheus, Grafana, Jaeger, Loki) via Homebrew and launchd.

## What Was Done

### Gemini Engine Instrumentation

The `structured-agent-gemini` crate now records metrics on every call to `GeminiEngine::request`. The `metrics` crate (facade, version 0.24) was added as a dependency. The following histograms are emitted, all labelled with `model`:

- `gemini.request.duration_ms` — wall-clock latency of each model call
- `gemini.context.message_count` — number of context messages sent per request
- `gemini.tokens.prompt` — prompt token count from `UsageMetadata`
- `gemini.tokens.output` — candidate token count
- `gemini.tokens.thoughts` — thinking token count
- `gemini.tokens.cached` — cached content token count (non-zero indicates a cache hit)

The `UsageMetadata` struct in `src/structured-agent-gemini/src/types.rs` was extended with `cached_content_token_count`, which maps to the `cachedContentTokenCount` field in the Gemini API response. This field is only present when context caching is active.

Each successful response is logged at `DEBUG` level via `tracing::debug!` with all token counts and duration, giving log-level visibility without requiring a metrics backend.

See `src/structured-agent-gemini/src/engine.rs` for the instrumentation site and `src/structured-agent-gemini/src/types.rs` for the updated `UsageMetadata` struct.

### Binary Telemetry Initialisation

`main.rs` was restructured. Previously it initialised the tracing subscriber before parsing arguments, which prevented conditional layers (Loki, OpenTelemetry) from being added based on runtime config. It now parses arguments first, builds the Tokio runtime manually, and initialises all telemetry inside the async context.

Three optional telemetry flags were added to `RunArgs`, `AcpArgs`, and `FileConfig`:

- `--metrics-port <PORT>` — starts a Prometheus scrape endpoint (via `metrics-exporter-prometheus`) on the given port; disabled if absent
- `--otlp-endpoint <URL>` — initialises an OpenTelemetry OTLP trace exporter pointing at Jaeger's gRPC collector; disabled if absent
- `--loki-url <URL>` — ships structured log events to Loki via `tracing-loki`; disabled if absent

A `TelemetryGuard` struct is returned from `init_telemetry` and held for the lifetime of `async_main`. Its `Drop` implementation calls `opentelemetry::global::shutdown_tracer_provider()`, ensuring spans are flushed on exit.

All three flags are also supported in the TOML config file:

```toml
metrics_port = 9091
otlp_endpoint = "http://localhost:4317"
loki_url = "http://localhost:3100"
```

The `metrics` crate facade means the gemini and vm crates do not need to know which exporter is in use; the binary crate installs the Prometheus recorder and the library crates emit against it transparently.

Relevant files: `src/structured-agent/src/main.rs`, `src/structured-agent/src/cli/args.rs`, `src/structured-agent/src/cli/config.rs`.

### Observability Scripts

A new `observability/` directory at the workspace root contains four shell scripts and the Grafana dashboard JSON. No sudo is required; all data lives under `~/.structured-agent/observability/` and all binaries are either Homebrew-managed or downloaded to `~/.structured-agent/bin/`.

`install.sh` taps `grafana/grafana` and installs Prometheus, Grafana, and Loki via Homebrew. Jaeger is downloaded from GitHub Releases to `~/.structured-agent/bin/jaeger`. Tempo was the original intended trace backend but has no macOS binaries in its GitHub releases and its `go.mod` uses `replace` directives that prevent `go install`; Jaeger v2 was used instead.

`setup.sh` generates all service configuration files and writes four launchd `LaunchAgent` plists to `~/Library/LaunchAgents/`, then loads them. It accepts port overrides via environment variables (`PROMETHEUS_PORT`, `GRAFANA_PORT`, `JAEGER_UI_PORT`, `JAEGER_OTLP_GRPC_PORT`, `JAEGER_OTLP_HTTP_PORT`, `LOKI_PORT`, `APP_METRICS_PORT`, `DATA_DIR`, `BIN_DIR`). It unloads existing agents before reloading, so it is safe to re-run.

`teardown.sh` unloads and removes all four plists. Data and config directories are left intact.

`status.sh` checks `launchctl list` for each agent and hits the HTTP health endpoints.

The launchd service labels are `com.structured-agent.{prometheus,grafana,jaeger,loki}`. Logs go to `~/Library/Logs/structured-agent/`.

### Grafana Dashboard

`observability/grafana/dashboards/structured-agent.json` is provisioned automatically by Grafana on startup. It contains eight panels covering request latency percentiles, request rate, average tokens per request broken down by type, cache hit rate, average cached tokens, context depth, prompt token distribution, and a Loki log stream. All metric panels use `$__rate_interval` for the range window and carry a `model` label variable.

The Grafana datasource provisioning configures Prometheus, Jaeger, and Loki with cross-linking: Loki's `derivedFields` maps trace IDs in log lines to Jaeger, and Jaeger's `tracesToLogsV2` links traces back to Loki.

The dashboard JSON path relative to the observability.rs source (if embedded via `include_str!`) is `../../../../observability/grafana/dashboards/structured-agent.json`.

## What Is Not Working

At the point this session ended, `setup.sh` had just been rewritten and not yet re-run. The session was cut off before verifying all four services came up healthy. The last recorded state was:

- Prometheus was healthy.
- Grafana was crashing on startup due to a YAML escape error in the datasources provisioning file (`\w` in a double-quoted YAML string is an unknown escape sequence). The fix — replacing `\w+` with `[0-9a-f]+` in the `matcherRegex` field — is present in the rewritten `setup.sh` but had not been applied to the running instance.
- Jaeger was crashing because the plist was passing v1-style CLI flags (`--collector.otlp.grpc.host-port`) that do not exist in v2. The fix — switching to `--config=file:<path>` with a generated `jaeger.yaml` — is in the rewritten `setup.sh` but had not been applied.
- Loki was running and had started successfully, though it emitted a disk usage warning (92% on the WAL volume).

The next step is to run `sh observability/setup.sh` from the workspace root and then `sh observability/status.sh` to verify all services are healthy.

## Dependency Versions

The OpenTelemetry crate family is sensitive to version alignment. The binary crate uses:

- `opentelemetry = "0.26"`
- `opentelemetry_sdk = "0.26"` with features `rt-tokio` and `trace`
- `opentelemetry-otlp = "0.26"` with features `grpc-tonic` and `trace`
- `tracing-opentelemetry = "0.27"`
- `tracing-loki = "0.2"` with features `compat-0-2-1` and `rustls`
- `metrics-exporter-prometheus = "0.16"`
- `tonic = "0.12"`

If any of these are bumped independently the build will likely break on trait or type mismatches. The `opentelemetry` family in particular has a tight coupling between the SDK, exporter, and `tracing-opentelemetry` versions.

## Dead Code and Cleanup

An `observability.rs` CLI module was created and then deleted after the `Observability` subcommand was removed from the `Command` enum. The deletion is clean; no references remain.

The `Alloy` package (`grafana/grafana/alloy`) was installed as a side-effect of an earlier `install.sh` run that referenced it before the script was corrected. It is not used by any configuration and can be removed with `brew uninstall alloy`.

The Jaeger binary at `/tmp/jaeger-2.17.0-darwin-amd64/` may still be present from a failed extraction attempt and can be removed.

## Possible Improvements

The `metrics` crate emits histograms with default buckets from `metrics-exporter-prometheus`. For latency in milliseconds, the default bucket boundaries are unlikely to be well-suited. Explicit buckets for `gemini.request.duration_ms` (e.g. 100, 500, 1000, 2000, 5000, 10000 ms) would give more useful percentile estimates and should be configured in the Prometheus builder at startup.

The `tracing-loki` integration logs all events to Loki regardless of level. A filter matching the `EnvFilter` used for the fmt layer would avoid shipping debug noise in production.

Jaeger v2 stores traces in memory by default (`max_traces: 100000`). Traces are lost on restart. For anything beyond ephemeral local development, the storage backend should be switched to Badger (local disk) or an external store. The Jaeger v2 config at `~/.structured-agent/observability/jaeger/jaeger.yaml` is the place to make that change.

The `setup.sh` script does not check whether services are already installed before attempting to configure them. Running it without first running `install.sh` will silently produce plists referencing non-existent binaries.

## References

- `src/structured-agent-gemini/src/engine.rs` — metrics instrumentation
- `src/structured-agent-gemini/src/types.rs` — `UsageMetadata` with `cached_content_token_count`
- `src/structured-agent/src/main.rs` — telemetry init and runtime restructuring
- `src/structured-agent/src/cli/args.rs` — `--metrics-port`, `--otlp-endpoint`, `--loki-url` flags
- `src/structured-agent/src/cli/config.rs` — `ObservabilityConfig`
- `observability/install.sh` — Homebrew installs and Jaeger binary download
- `observability/setup.sh` — config generation and launchd registration
- `observability/grafana/dashboards/structured-agent.json` — Grafana dashboard
- https://github.com/jaegertracing/jaeger/releases/tag/v2.17.0
- https://grafana.com/docs/grafana/latest/administration/provisioning/
- https://docs.rs/metrics/latest/metrics/
- https://docs.rs/metrics-exporter-prometheus/latest/metrics_exporter_prometheus/
