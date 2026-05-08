#!/bin/sh
set -e

PROMETHEUS_PORT="${PROMETHEUS_PORT:-9090}"
GRAFANA_PORT="${GRAFANA_PORT:-3000}"
JAEGER_OTLP_GRPC_PORT="${JAEGER_OTLP_GRPC_PORT:-4317}"
JAEGER_OTLP_HTTP_PORT="${JAEGER_OTLP_HTTP_PORT:-4318}"
JAEGER_UI_PORT="${JAEGER_UI_PORT:-16686}"
LOKI_PORT="${LOKI_PORT:-3100}"
APP_METRICS_PORT="${APP_METRICS_PORT:-9091}"
DATA_DIR="${DATA_DIR:-$HOME/.structured-agent/observability}"
BIN_DIR="${BIN_DIR:-$HOME/.structured-agent/bin}"

BREW_PREFIX="$(brew --prefix)"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
LOG_DIR="$HOME/Library/Logs/structured-agent"
LAUNCH_AGENTS_DIR="$HOME/Library/LaunchAgents"

printf 'Creating directories...\n'
mkdir -p \
  "$DATA_DIR/prometheus" \
  "$DATA_DIR/jaeger" \
  "$DATA_DIR/loki/data/chunks" \
  "$DATA_DIR/loki/data/rules" \
  "$DATA_DIR/grafana/data" \
  "$DATA_DIR/grafana/logs" \
  "$DATA_DIR/grafana/provisioning/datasources" \
  "$DATA_DIR/grafana/provisioning/dashboards/json" \
  "$LOG_DIR" \
  "$LAUNCH_AGENTS_DIR"

printf 'Writing prometheus.yml...\n'
printf 'global:\n  scrape_interval: 5s\n\nscrape_configs:\n  - job_name: prometheus\n    static_configs:\n      - targets: ['\''localhost:%s'\'']\n  - job_name: structured-agent\n    static_configs:\n      - targets: ['\''localhost:%s'\'']\n' \
  "$PROMETHEUS_PORT" "$APP_METRICS_PORT" \
  > "$DATA_DIR/prometheus/prometheus.yml"

printf 'Writing jaeger.yaml...\n'
printf 'service:\n  extensions: [jaeger_storage, jaeger_query]\n  pipelines:\n    traces:\n      receivers: [otlp]\n      processors: [batch]\n      exporters: [jaeger_storage_exporter]\nextensions:\n  jaeger_query:\n    http:\n      endpoint: 0.0.0.0:%s\n    storage:\n      traces: memstore\n  jaeger_storage:\n    backends:\n      memstore:\n        memory:\n          max_traces: 100000\nreceivers:\n  otlp:\n    protocols:\n      grpc:\n        endpoint: 0.0.0.0:%s\n      http:\n        endpoint: 0.0.0.0:%s\nprocessors:\n  batch:\nexporters:\n  jaeger_storage_exporter:\n    trace_storage: memstore\n' \
  "$JAEGER_UI_PORT" "$JAEGER_OTLP_GRPC_PORT" "$JAEGER_OTLP_HTTP_PORT" \
  > "$DATA_DIR/jaeger/jaeger.yaml"

printf 'Writing loki.yaml...\n'
printf 'auth_enabled: false\nserver:\n  http_listen_port: %s\ncommon:\n  instance_addr: 127.0.0.1\n  path_prefix: %s/loki/data\n  storage:\n    filesystem:\n      chunks_directory: %s/loki/data/chunks\n      rules_directory: %s/loki/data/rules\n  replication_factor: 1\n  ring:\n    kvstore:\n      store: inmemory\nschema_config:\n  configs:\n    - from: 2020-10-24\n      store: tsdb\n      object_store: filesystem\n      schema: v13\n      index:\n        prefix: index_\n        period: 24h\n' \
  "$LOKI_PORT" "$DATA_DIR" "$DATA_DIR" "$DATA_DIR" \
  > "$DATA_DIR/loki/loki.yaml"

printf 'Writing datasources.yaml...\n'
printf 'apiVersion: 1\ndatasources:\n  - name: Prometheus\n    type: prometheus\n    uid: prometheus\n    access: proxy\n    url: http://localhost:%s\n    isDefault: true\n    jsonData:\n      timeInterval: 15s\n  - name: Jaeger\n    type: jaeger\n    uid: jaeger\n    access: proxy\n    url: http://localhost:%s\n    jsonData:\n      tracesToLogsV2:\n        datasourceUid: loki\n        filterByTraceID: true\n  - name: Loki\n    type: loki\n    uid: loki\n    access: proxy\n    url: http://localhost:%s\n    jsonData:\n      derivedFields:\n        - datasourceUid: jaeger\n          matcherRegex: "traceID=([0-9a-f]+)"\n          name: TraceID\n          url: "${__value.raw}"\n' \
  "$PROMETHEUS_PORT" "$JAEGER_UI_PORT" "$LOKI_PORT" \
  > "$DATA_DIR/grafana/provisioning/datasources/datasources.yaml"

printf 'Writing dashboards.yaml...\n'
printf 'apiVersion: 1\nproviders:\n  - name: structured-agent\n    orgId: 1\n    folder: Structured Agent\n    type: file\n    disableDeletion: false\n    updateIntervalSeconds: 10\n    allowUiUpdates: true\n    options:\n      path: %s/grafana/provisioning/dashboards/json\n' \
  "$DATA_DIR" \
  > "$DATA_DIR/grafana/provisioning/dashboards/dashboards.yaml"

printf 'Copying dashboard JSON...\n'
cp "$SCRIPT_DIR/grafana/dashboards/structured-agent.json" \
   "$DATA_DIR/grafana/provisioning/dashboards/json/structured-agent.json"

printf 'Writing grafana.ini...\n'
printf '[server]\nhttp_port = %s\n[paths]\ndata = %s/grafana/data\nlogs = %s/grafana/logs\nprovisioning = %s/grafana/provisioning\n[auth.anonymous]\nenabled = true\norg_role = Admin\n' \
  "$GRAFANA_PORT" "$DATA_DIR" "$DATA_DIR" "$DATA_DIR" \
  > "$DATA_DIR/grafana/grafana.ini"

printf 'Writing launchd plists...\n'

printf '<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key>
  <string>com.structured-agent.prometheus</string>
  <key>ProgramArguments</key>
  <array>
    <string>%s/bin/prometheus</string>
    <string>--config.file=%s/prometheus/prometheus.yml</string>
    <string>--storage.tsdb.path=%s/prometheus/data</string>
    <string>--web.listen-address=:%s</string>
  </array>
  <key>WorkingDirectory</key>
  <string>%s/prometheus</string>
  <key>KeepAlive</key>
  <true/>
  <key>RunAtLoad</key>
  <true/>
  <key>StandardOutPath</key>
  <string>%s/prometheus.log</string>
  <key>StandardErrorPath</key>
  <string>%s/prometheus.log</string>
</dict>
</plist>
' "$BREW_PREFIX" "$DATA_DIR" "$DATA_DIR" "$PROMETHEUS_PORT" "$DATA_DIR" "$LOG_DIR" "$LOG_DIR" \
  > "$LAUNCH_AGENTS_DIR/com.structured-agent.prometheus.plist"

printf '<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key>
  <string>com.structured-agent.grafana</string>
  <key>ProgramArguments</key>
  <array>
    <string>%s/bin/grafana</string>
    <string>server</string>
    <string>--homepath</string>
    <string>%s/share/grafana</string>
    <string>--config</string>
    <string>%s/grafana/grafana.ini</string>
  </array>
  <key>WorkingDirectory</key>
  <string>%s/grafana</string>
  <key>KeepAlive</key>
  <true/>
  <key>RunAtLoad</key>
  <true/>
  <key>StandardOutPath</key>
  <string>%s/grafana.log</string>
  <key>StandardErrorPath</key>
  <string>%s/grafana.log</string>
</dict>
</plist>
' "$BREW_PREFIX" "$BREW_PREFIX" "$DATA_DIR" "$DATA_DIR" "$LOG_DIR" "$LOG_DIR" \
  > "$LAUNCH_AGENTS_DIR/com.structured-agent.grafana.plist"

printf '<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key>
  <string>com.structured-agent.jaeger</string>
  <key>ProgramArguments</key>
  <array>
    <string>%s/jaeger</string>
    <string>--config=file:%s/jaeger/jaeger.yaml</string>
  </array>
  <key>WorkingDirectory</key>
  <string>%s/jaeger</string>
  <key>KeepAlive</key>
  <true/>
  <key>RunAtLoad</key>
  <true/>
  <key>StandardOutPath</key>
  <string>%s/jaeger.log</string>
  <key>StandardErrorPath</key>
  <string>%s/jaeger.log</string>
</dict>
</plist>
' "$BIN_DIR" "$DATA_DIR" "$DATA_DIR" "$LOG_DIR" "$LOG_DIR" \
  > "$LAUNCH_AGENTS_DIR/com.structured-agent.jaeger.plist"

printf '<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key>
  <string>com.structured-agent.loki</string>
  <key>ProgramArguments</key>
  <array>
    <string>%s/bin/loki</string>
    <string>-config.file=%s/loki/loki.yaml</string>
  </array>
  <key>WorkingDirectory</key>
  <string>%s/loki</string>
  <key>KeepAlive</key>
  <true/>
  <key>RunAtLoad</key>
  <true/>
  <key>StandardOutPath</key>
  <string>%s/loki.log</string>
  <key>StandardErrorPath</key>
  <string>%s/loki.log</string>
</dict>
</plist>
' "$BREW_PREFIX" "$DATA_DIR" "$DATA_DIR" "$LOG_DIR" "$LOG_DIR" \
  > "$LAUNCH_AGENTS_DIR/com.structured-agent.loki.plist"

printf 'Reloading launchd agents...\n'
for SERVICE in prometheus grafana jaeger loki; do
  PLIST="$LAUNCH_AGENTS_DIR/com.structured-agent.$SERVICE.plist"
  launchctl unload "$PLIST" 2>/dev/null || true
  launchctl load "$PLIST"
done

printf '\nSetup complete.\n'
printf '  Grafana:    http://localhost:%s  (admin/admin)\n' "$GRAFANA_PORT"
printf '  Prometheus: http://localhost:%s\n' "$PROMETHEUS_PORT"
printf '  Jaeger:     http://localhost:%s\n' "$JAEGER_UI_PORT"
printf '  Loki:       http://localhost:%s\n' "$LOKI_PORT"
printf '\nRun ./status.sh to verify all services are healthy.\n'
