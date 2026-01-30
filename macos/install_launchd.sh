#!/bin/bash
set -euo pipefail

PROJECT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )/.."
cd "$PROJECT_DIR"

LAUNCH_AGENTS_DIR="$HOME/Library/LaunchAgents"
LOG_DIR="$HOME/Library/Logs"
mkdir -p "$LAUNCH_AGENTS_DIR" "$LOG_DIR"

# Ensure wrappers are executable
chmod +x macos/run_api.sh macos/run_telegram_bot.sh

render_plist() {
  local template="$1"
  local out="$2"
  sed \
    -e "s|__PROJECT_DIR__|$PROJECT_DIR|g" \
    -e "s|__LOG_DIR__|$LOG_DIR|g" \
    "$template" > "$out"
}

API_PLIST="$LAUNCH_AGENTS_DIR/com.onchainbeast.api.plist"
BOT_PLIST="$LAUNCH_AGENTS_DIR/com.onchainbeast.telegram_bot.plist"

render_plist "macos/com.onchainbeast.api.plist.template" "$API_PLIST"
render_plist "macos/com.onchainbeast.telegram_bot.plist.template" "$BOT_PLIST"

# Unload existing (ignore errors)
launchctl bootout gui/"$(id -u)" "$API_PLIST" >/dev/null 2>&1 || true
launchctl bootout gui/"$(id -u)" "$BOT_PLIST" >/dev/null 2>&1 || true

# Load new
launchctl bootstrap gui/"$(id -u)" "$API_PLIST"
launchctl bootstrap gui/"$(id -u)" "$BOT_PLIST"

# Kickstart immediately
launchctl kickstart -k gui/"$(id -u)"/com.onchainbeast.api || true
launchctl kickstart -k gui/"$(id -u)"/com.onchainbeast.telegram_bot || true

echo "✅ Installed LaunchAgents:"
echo "  - com.onchainbeast.api"
echo "  - com.onchainbeast.telegram_bot"
echo ""
echo "Logs:"
echo "  $LOG_DIR/onchainbeast_api.out.log"
echo "  $LOG_DIR/onchainbeast_api.err.log"
echo "  $LOG_DIR/onchainbeast_telegram_bot.out.log"
echo "  $LOG_DIR/onchainbeast_telegram_bot.err.log"
echo ""
echo "Check status:"
echo "  launchctl print gui/$(id -u)/com.onchainbeast.api"
echo "  launchctl print gui/$(id -u)/com.onchainbeast.telegram_bot"
