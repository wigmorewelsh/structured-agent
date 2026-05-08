#!/bin/sh
set -e

printf 'Tapping grafana/grafana...\n'
brew tap grafana/grafana

printf 'Installing prometheus...\n'
brew install prometheus

printf 'Installing grafana...\n'
brew install grafana

printf 'Installing tempo...\n'
brew install grafana/grafana/tempo

printf 'Installing loki...\n'
brew install grafana/grafana/loki

printf 'All packages installed.\n'
printf 'Run ./setup.sh to configure and start services.\n'
