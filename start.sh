#!/bin/bash
# Start OnChain Beast (side-wallet tracer)

set -euo pipefail

PROJECT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
cd "$PROJECT_DIR"

echo "Starting OnChain Beast..."

# Load environment variables from .env if present
if [ -f .env ]; then
  set -a
  # shellcheck disable=SC1091
  . ./.env
  set +a
fi

API_HOST="${API_HOST:-127.0.0.1}"
API_PORT="${API_PORT:-8080}"

echo "API: http://${API_HOST}:${API_PORT}"

DATABASE_URL="${DATABASE_URL:-}"
if [[ "${DATABASE_URL}" == "memory" || "${DATABASE_URL}" == memory:* ]]; then
  echo "Storage: in-memory (no Postgres required)"
else
  echo "Checking PostgreSQL..."
  if ! command -v psql >/dev/null 2>&1; then
    echo "PostgreSQL not found. Either install it, or set DATABASE_URL=memory in .env"
    exit 1
  fi

  if command -v pg_isready >/dev/null 2>&1; then
    if ! pg_isready -q; then
      echo "PostgreSQL not responding."
      exit 1
    fi
  fi
fi

if command -v lsof >/dev/null 2>&1; then
  EXISTING_PID="$(lsof -ti tcp:${API_PORT} 2>/dev/null | head -n 1 || true)"
  if [ -n "$EXISTING_PID" ]; then
    echo "API already running on ${API_HOST}:${API_PORT} (pid ${EXISTING_PID})"
    echo "Health: curl http://${API_HOST}:${API_PORT}/health"
    exit 0
  fi
fi

if [ ! -x ./target/release/onchain_beast ]; then
  echo "Release binary not found. Building..."
  cargo build --release
fi

echo "Starting..."
exec ./target/release/onchain_beast
