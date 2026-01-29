# OnChain Beast 🚀

A powerful Solana blockchain analysis engine designed to revolutionize onchain investigation.

## Overview

**OnChain Beast** is a sophisticated tool for deep onchain analysis on the Solana blockchain. It helps investigators:

- 🔗 **Find Connected Wallets**: Discover side wallets and alternate addresses belonging to the same entity
- 🔍 **Track Fund Flows**: Follow funds even through exchange intermediaries
- 📊 **Detect Patterns**: Identify behavioral signatures and suspicious activities (P&D, wash trading, etc.)
- 🛡️ **Risk Assessment**: Evaluate wallet risk profiles with advanced heuristics
- 🔐 **Mixer Detection**: Identify when wallets use exchanges as mixing services

## Architecture

```
src/
├── main.rs              # Application entry point
├── modules/             # Core analysis modules
│   ├── wallet_tracker.rs       # Wallet clustering & relationship detection
│   ├── transaction_analyzer.rs # Transaction-level analysis
│   ├── pattern_detector.rs     # Behavioral pattern recognition
│   └── exchange_detector.rs    # Exchange interaction tracking
├── core/                # Core infrastructure
│   ├── rpc_client.rs    # Solana RPC interactions
│   ├── config.rs        # Configuration management
│   └── errors.rs        # Error types
├── database/            # Data persistence
│   └── storage.rs       # Database operations
├── api/                 # API handlers
│   ├── handlers.rs      # Request handlers
│   └── responses.rs     # Response types
└── analysis/            # Analysis engine orchestration
    └── mod.rs           # Main analysis pipeline
```

## Key Features

### 1. **Wallet Tracker**
- Identifies connected wallets through temporal and behavioral analysis
- Builds wallet relationship graphs
- Clusters wallets likely belonging to the same entity

### 2. **Transaction Analyzer**
- Deep transaction-level analysis
- Fund flow tracking
- Anomaly detection in transaction patterns

### 3. **Pattern Detector**
- Pump & dump detection
- Wash trading identification
- Behavioral fingerprinting
- Similar wallet pattern matching

### 4. **Exchange Detector**
- Known exchange address database
- Mixer behavior detection
- Fund tracing through exchange wallets
- Identifies withdrawal patterns

## Setup

### Prerequisites
- Rust 1.93.0 or later
- Solana CLI (optional, for additional tools)

### Installation

```bash
cd onchain_beast
cargo build --release
```

### Configuration

Use the provided `.env.example` (recommended):
```bash
cp .env.example .env
```

Key settings (defaults shown):
- `SOLANA_RPC_ENDPOINT=https://api.mainnet-beta.solana.com`
- `DATABASE_URL=postgresql://<user>@localhost/onchain_beast_personal`
- `REDIS_URL=redis://127.0.0.1:6379`
- `SERVER_HOST=127.0.0.1`
- `SERVER_PORT=8080`

### Running the API Server

```bash
./start.sh
```

Health check:
```bash
curl http://127.0.0.1:8080/health
```

## Usage Examples

### Analyze a Wallet (API)

```bash
curl http://127.0.0.1:8080/analysis/wallet/<WALLET_ADDRESS>
```

### Wallet Stats (API)

```bash
curl http://127.0.0.1:8080/transfer/wallet-stats/<WALLET_ADDRESS>
```

### Token Metadata (API)

```bash
curl http://127.0.0.1:8080/metadata/token/<MINT_ADDRESS>
```

### Telegram Bot (Optional)

```bash
export TELEGRAM_BOT_TOKEN="<YOUR_TOKEN>"
export ONCHAIN_BEAST_API_BASE="http://127.0.0.1:8080"  # optional
./target/release/telegram_bot
```

## Next Optional Enhancements

- API authentication (JWT / API keys)
- TLS/HTTPS termination
- WebSocket streaming
- Dashboard UI

## Performance Characteristics

- **Async/Await Runtime**: Full async support using Tokio
- **Concurrent Analysis**: Process multiple wallets simultaneously
- **Memory Efficient**: Optimized for large-scale blockchain data
- **Type Safe**: Rust's type system prevents entire categories of bugs
- **Zero-Cost Abstractions**: No runtime overhead from high-level constructs

## Security

- Memory safety guaranteed by Rust
- No null pointer dereferences
- No data races
- All financial calculations use checked arithmetic

## Contributing

Contributions welcome! Please ensure:
- Code passes `cargo fmt` and `cargo clippy`
- All tests pass: `cargo test`
- New features include documentation

## License

MIT

## Contact & Support

For questions or issues about OnChain Beast, please open an issue or reach out.

---

**OnChain Beast** - Changing the game for onchain investigations 🚀
