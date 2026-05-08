#!/bin/sh
set -e

PROMETHEUS_PORT="${PROMETHEUS_PORT:-9090}"
GRAFANA_PORT="${GRAFANA_PORT:-3000}"
TEMPO_HTTP_PORT="${TEMPO_HTTP_PORT:-3200}"
LOKI_PORT="${LOKI_PORT:-3100}"

check_launchd() {
  SERVICE="$1"
  LABEL="com.structured-agent.$SERVICE"
  if launchctl list "$LABEL" >/dev/null 2>&1; then
    PID="$(launchctl list "$LABEL" | grep '"PID"' | tr -d ' "PID=:,')"
    if [ -n "$PID" ]; then
      printf '  %-12s launchd: running (PID %s)\n' "$SERVICE" "$PID"
    else
      printf '  %-12s launchd: loaded (not running)\n' "$SERVICE"
    fi
  else
    printf '  %-12s launchd: stopped\n' "$SERVICE"
  fi
}

check_http() {
  SERVICE="$1"
  URL="$2"
  if curl -sf "$URL" >/dev/null 2>&1; then
    printf '  %-12s http:    healthy (%s)\n' "$SERVICE" "$URL"
  else
    printf '  %-12s http:    unreachable (%s)\n' "$SERVICE" "$URL"
  fi
}

printf 'Launchd agent status:\n'
check_launchd prometheus
check_launchd grafana
check_launchd tempo
check_launchd loki

printf '\nHTTP health checks:\n'
check_http prometheus "http://localhost:$PROMETHEUS_PORT/-/healthy"
check_http grafana    "http://localhost:$GRAFANA_PORT/api/health"
check_http tempo      "http://localhost:$TEMPO_HTTP_PORT/ready"
check_http loki       "http://localhost:$LOKI_PORT/ready"
