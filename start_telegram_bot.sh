#!/bin/bash
# Start OnChain Beast Telegram bot

set -euo pipefail

PROJECT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
cd "$PROJECT_DIR"

# Load environment variables
if [ -f .env ]; then
  set -a
  # shellcheck disable=SC1091
  . ./.env
  set +a
fi

: "${ONCHAIN_BEAST_API_BASE:=http://127.0.0.1:8080}"
export ONCHAIN_BEAST_API_BASE

if [ -z "${TELEGRAM_BOT_TOKEN:-}" ]; then
  echo "❌ TELEGRAM_BOT_TOKEN is not set."
  echo "   Set it in .env (recommended) or your shell environment."
  echo "   Example (.env): TELEGRAM_BOT_TOKEN=..."
  exit 1
fi

if [ ! -x ./target/release/telegram_bot ]; then
  echo "⚙️  telegram_bot binary not found. Building..."
  cargo build --release --bin telegram_bot
  echo "✅ Build complete"
fi

if command -v pgrep >/dev/null 2>&1; then
  if pgrep -f "target/release/telegram_bot" >/dev/null 2>&1; then
    echo "ℹ️  telegram_bot already running. Stop it first (or you'll hit Telegram getUpdates conflicts)."
    echo "   Try: pkill -f target/release/telegram_bot"
    exit 0
  fi
fi

echo "🤖 Starting Telegram bot (API base: $ONCHAIN_BEAST_API_BASE)"
exec ./target/release/telegram_bot
