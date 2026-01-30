#!/bin/bash
set -euo pipefail

PROJECT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )/.."
cd "$PROJECT_DIR"

# Load env (optional)
if [ -f .env ]; then
  set -a
  # shellcheck disable=SC1091
  . ./.env
  set +a
fi

: "${SERVER_HOST:=127.0.0.1}"
: "${SERVER_PORT:=8080}"
: "${METRICS_PORT:=9090}"

# Build if missing
if [ ! -x ./target/release/onchain_beast ]; then
  cargo build --release
fi

exec ./target/release/onchain_beast
