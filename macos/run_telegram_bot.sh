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
: "${ONCHAIN_BEAST_API_BASE:=http://${SERVER_HOST}:${SERVER_PORT}}"
export ONCHAIN_BEAST_API_BASE

if [ -z "${TELEGRAM_BOT_TOKEN:-}" ]; then
  echo "❌ TELEGRAM_BOT_TOKEN is not set. Put it in .env (gitignored) or your shell env." >&2
  exit 1
fi

# Wait for API health (up to ~60s)
health_url="http://${SERVER_HOST}:${SERVER_PORT}/health"
for _ in {1..60}; do
  code="$(curl -s -o /dev/null -w "%{http_code}" "$health_url" || true)"
  if [ "$code" = "200" ]; then
    break
  fi
  sleep 1
done

# Build if missing
if [ ! -x ./target/release/telegram_bot ]; then
  cargo build --release --bin telegram_bot
fi

exec ./target/release/telegram_bot
