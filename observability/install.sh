#!/bin/sh
set -e

JAEGER_VERSION="2.17.0"

printf 'Tapping grafana/grafana...\n'
brew tap grafana/grafana

printf 'Installing prometheus...\n'
brew install prometheus

printf 'Installing grafana...\n'
brew install grafana

printf 'Installing loki...\n'
brew install loki

printf 'Installing Jaeger %s...\n' "$JAEGER_VERSION"
ARCH="$(uname -m)"
case "$ARCH" in
  arm64)  JAEGER_ARCH="arm64" ;;
  x86_64) JAEGER_ARCH="amd64" ;;
  *)
    printf 'Unsupported architecture: %s\n' "$ARCH"
    exit 1
    ;;
esac

JAEGER_URL="https://github.com/jaegertracing/jaeger/releases/download/v${JAEGER_VERSION}/jaeger-${JAEGER_VERSION}-darwin-${JAEGER_ARCH}.tar.gz"
BIN_DIR="$HOME/.structured-agent/bin"
mkdir -p "$BIN_DIR"

curl -L "$JAEGER_URL" -o /tmp/jaeger.tar.gz
tar -xzf /tmp/jaeger.tar.gz -C /tmp
mv "/tmp/jaeger-${JAEGER_VERSION}-darwin-${JAEGER_ARCH}/jaeger" "$BIN_DIR/jaeger"
chmod +x "$BIN_DIR/jaeger"
rm -rf /tmp/jaeger.tar.gz "/tmp/jaeger-${JAEGER_VERSION}-darwin-${JAEGER_ARCH}"

printf 'Jaeger installed to %s/jaeger\n' "$BIN_DIR"
printf '\nAll packages installed.\n'
printf 'Run ./setup.sh to configure and start services.\n'
