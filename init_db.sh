#!/usr/bin/env bash
# Initialize PostgreSQL database for OnChain Beast (side-wallet tracer).

set -euo pipefail

echo "Initializing PostgreSQL for OnChain Beast (side-wallet tracer)..."
echo "============================================================="
echo ""

CURRENT_USER="${CURRENT_USER:-$(whoami)}"
DB_NAME="${DB_NAME:-onchain_beast_personal}"

echo "Current user: ${CURRENT_USER}"
echo "Database name: ${DB_NAME}"
echo ""

if ! command -v psql >/dev/null 2>&1; then
  echo "PostgreSQL client (psql) not found. Install PostgreSQL first."
  exit 1
fi

echo "Checking PostgreSQL status..."
if command -v pg_isready >/dev/null 2>&1; then
  if ! pg_isready -q; then
    if command -v brew >/dev/null 2>&1; then
      echo "PostgreSQL not responding. Trying: brew services start postgresql"
      brew services start postgresql >/dev/null 2>&1 || true
      sleep 2
    fi
  fi
  if ! pg_isready -q; then
    echo "PostgreSQL still not responding. Start it and re-run this script."
    exit 1
  fi
else
  # Fallback check if pg_isready is not available.
  if ! psql -d postgres -c "SELECT 1" >/dev/null 2>&1; then
    echo "PostgreSQL not responding. Start it and re-run this script."
    exit 1
  fi
fi

echo "Setting up PostgreSQL role (idempotent)..."
psql -d postgres -c "CREATE USER ${CURRENT_USER} WITH CREATEDB SUPERUSER;" 2>/dev/null || true

echo "Creating database (idempotent)..."
psql -U "${CURRENT_USER}" -d postgres -c "CREATE DATABASE ${DB_NAME};" 2>/dev/null || true

echo ""
echo "Database initialization complete."
echo ""
echo "Connection string:"
echo "  postgresql://${CURRENT_USER}@localhost/${DB_NAME}"
echo ""
echo "Next: start the API server (it will create tables automatically):"
echo "  ./start.sh"
