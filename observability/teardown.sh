#!/bin/sh
set -e

LAUNCH_AGENTS_DIR="$HOME/Library/LaunchAgents"
DATA_DIR="${DATA_DIR:-$HOME/.structured-agent/observability}"

for SERVICE in prometheus grafana tempo loki; do
  PLIST="$LAUNCH_AGENTS_DIR/com.structured-agent.$SERVICE.plist"
  launchctl unload "$PLIST" 2>/dev/null || true
  rm -f "$PLIST"
  printf 'Removed %s\n' "$PLIST"
done

printf '\nAll agents unloaded and plists removed.\n'
printf 'Data and config directories were NOT deleted.\n'
printf 'They remain at: %s\n' "$DATA_DIR"
printf 'Logs remain at: %s\n' "$HOME/Library/Logs/structured-agent"
