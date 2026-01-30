#!/bin/bash
set -euo pipefail

LAUNCH_AGENTS_DIR="$HOME/Library/LaunchAgents"
API_PLIST="$LAUNCH_AGENTS_DIR/com.onchainbeast.api.plist"
BOT_PLIST="$LAUNCH_AGENTS_DIR/com.onchainbeast.telegram_bot.plist"

launchctl bootout gui/"$(id -u)" "$BOT_PLIST" >/dev/null 2>&1 || true
launchctl bootout gui/"$(id -u)" "$API_PLIST" >/dev/null 2>&1 || true

rm -f "$BOT_PLIST" "$API_PLIST"

echo "✅ Uninstalled LaunchAgents (removed plists):"
echo "  - com.onchainbeast.api"
echo "  - com.onchainbeast.telegram_bot"
