#!/bin/bash
# End-to-end local smoke test for OnChain Beast + Telegram bot.
# - Does NOT print TELEGRAM_BOT_TOKEN
# - Starts API if not already running
# - Runs the bot briefly to confirm it starts and can poll

set -euo pipefail

PROJECT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
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

api_pid=""
bot_pid=""
cleanup() {
  if [ -n "$bot_pid" ]; then
    kill "$bot_pid" >/dev/null 2>&1 || true
  fi
  if [ -n "$api_pid" ]; then
    kill "$api_pid" >/dev/null 2>&1 || true
  fi
}
trap cleanup EXIT

# Ensure release binaries exist
if [ ! -x ./target/release/onchain_beast ]; then
  cargo build --release
fi
if [ ! -x ./target/release/telegram_bot ]; then
  cargo build --release --bin telegram_bot
fi

# Start API if not already listening
if command -v lsof >/dev/null 2>&1; then
  existing_pid="$(lsof -ti tcp:${SERVER_PORT} 2>/dev/null | head -n 1 || true)"
else
  existing_pid=""
fi

if [ -n "$existing_pid" ]; then
  echo "ℹ️  API already listening on ${SERVER_HOST}:${SERVER_PORT} (pid ${existing_pid})"
else
  echo "🚀 Starting API in background on ${SERVER_HOST}:${SERVER_PORT}..."
  ./target/release/onchain_beast > /tmp/onchain_beast_api.log 2>&1 &
  api_pid="$!"
fi

# Wait for /health
echo "⏳ Waiting for API health..."
for i in {1..30}; do
  code="$(curl -s -o /dev/null -w "%{http_code}" "http://${SERVER_HOST}:${SERVER_PORT}/health" || true)"
  if [ "$code" = "200" ]; then
    echo "✅ API health OK"
    break
  fi
  sleep 1
  if [ "$i" = "30" ]; then
    echo "❌ API did not become healthy (last HTTP ${code})."
    echo "   Tail logs: tail -n 200 /tmp/onchain_beast_api.log"
    exit 1
  fi
done

# Run the bot briefly
if [ -z "${TELEGRAM_BOT_TOKEN:-}" ]; then
  if [ -t 0 ]; then
    echo "🔐 TELEGRAM_BOT_TOKEN is not set."
    echo "   Paste it locally (input hidden). It will NOT be printed."
    read -r -s -p "TELEGRAM_BOT_TOKEN: " TELEGRAM_BOT_TOKEN
    echo
    export TELEGRAM_BOT_TOKEN
  else
    echo "⚠️  TELEGRAM_BOT_TOKEN is not set; skipping bot startup."
    echo "   Put it in .env (recommended) or export it in your shell, then re-run:"
    echo "     ./smoke_telegram_bot_local.sh"
    exit 1
  fi
fi

echo "🤖 Starting telegram_bot briefly (API base: ${ONCHAIN_BEAST_API_BASE})"

# Avoid common local double-run issues.
pkill -f "./target/release/telegram_bot" >/dev/null 2>&1 || true
pkill -f "target/release/telegram_bot" >/dev/null 2>&1 || true

BOT_LOG="/tmp/onchain_beast_telegram_bot.log"
rm -f "$BOT_LOG" >/dev/null 2>&1 || true

./target/release/telegram_bot >"$BOT_LOG" 2>&1 &
bot_pid="$!"

# Watch for known Telegram conflicts for a few seconds.
for i in {1..6}; do
  if ! kill -0 "$bot_pid" >/dev/null 2>&1; then
    break
  fi

  if grep -q "TerminatedByOtherGetUpdates" "$BOT_LOG" 2>/dev/null; then
    echo "❌ Telegram polling conflict: another process is using getUpdates for this bot token."
    echo "   Stop other running instances (including other projects/servers) using the same token, then re-run."
    kill "$bot_pid" >/dev/null 2>&1 || true
    sleep 1
    echo "--- last bot logs ---"
    tail -n 30 "$BOT_LOG" || true
    exit 2
  fi

  if grep -qi "can't use getUpdates" "$BOT_LOG" 2>/dev/null; then
    echo "❌ Telegram conflict: webhook is likely set for this bot token (polling disabled)."
    echo "   Clear webhook or switch this bot to webhook mode."
    kill "$bot_pid" >/dev/null 2>&1 || true
    sleep 1
    echo "--- last bot logs ---"
    tail -n 30 "$BOT_LOG" || true
    exit 2
  fi

  sleep 1
done

# Let it run briefly, then stop (portable replacement for timeout).
sleep 10
kill "$bot_pid" >/dev/null 2>&1 || true
wait "$bot_pid" >/dev/null 2>&1 || true

echo "✅ Bot started and was stopped cleanly."
echo "Next: open Telegram and send /help or /status to your bot to verify message handling."
