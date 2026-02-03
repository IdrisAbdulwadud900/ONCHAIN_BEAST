# OnChain Beast

Side-wallet tracing for the Solana blockchain, exposed as a minimal REST API and a Telegram bot.

This project ingests transfers for a wallet, builds a wallet relationship graph, and returns likely
side-wallet candidates. It also includes a best-effort "CEX hop" heuristic to surface wallets that
may have been funded after the target wallet sent assets to a centralized exchange (exchanges pool
funds, so this is probabilistic and provided with evidence + confidence scores).

## API

- `GET /health`
- `GET /api/v1/wallet/{address}/side-wallets`

Query params for `side-wallets`:
- `bootstrap=true|false` (default: `true`) - ingest recent txs for the target wallet first
- `bootstrap_limit=25` - how many signatures to ingest for the target wallet
- `depth=2` - relationship graph expansion depth
- `threshold=0.10` - minimum score
- `limit=15` - max candidates returned
- `lookback_days=30` - event-evidence window
- `cex_hops=true|false` (default: `true`) - enable CEX-hop heuristic
- `cex_bootstrap_limit=15` - extra ingestion for intermediary wallets (deposit/hot wallets)

## Running

Requirements:
- Rust toolchain
- PostgreSQL (optional — see `DATABASE_URL=memory` below)

Environment variables:
- `SOLANA_RPC_ENDPOINT` (default: Solana mainnet RPC)
- `DATABASE_URL`
  - `memory` (default) - in-memory, no Postgres required
  - `postgresql://...` - persistent storage
- `API_HOST` (default: `127.0.0.1`)
- `API_PORT` (default: `8080`)
- `API_KEYS` (optional, comma-separated). If set, requests must include `X-API-Key`.

Start:
```bash
cargo build --release
./target/release/onchain_beast
```

## Telegram Bot

Build:
```bash
cargo build --release --bin telegram_bot
```

Run:
```bash
export TELEGRAM_BOT_TOKEN="..."
export ONCHAIN_BEAST_API_BASE="http://127.0.0.1:8080"
export ONCHAIN_BEAST_API_KEY="..."   # only if API_KEYS is set on the server
./target/release/telegram_bot
```

## Important Note About CEX Hops

Centralized exchanges aggregate and pool funds. The "CEX hop" results are heuristics and should be
treated as investigative leads, not definitive attribution.
