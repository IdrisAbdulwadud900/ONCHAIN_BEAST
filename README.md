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

Set environment variables:
```bash
export SOLANA_RPC_ENDPOINT="https://api.mainnet-beta.solana.com"
export DATABASE_URL="sqlite:onchain_beast.db"
export MAX_CONCURRENT_REQUESTS=100
export CACHE_TTL_SECONDS=3600
```

### Running

```bash
cargo run --release
```

## Usage Examples

### Analyze a Wallet

```rust
use onchain_beast::analysis::AnalysisEngine;

#[tokio::main]
async fn main() {
    let engine = AnalysisEngine::new();
    
    let result = engine.investigate_wallet(
        "YOUR_WALLET_ADDRESS"
    ).await.unwrap();
    
    println!("Connected wallets: {:?}", result.side_wallets);
    println!("Risk Level: {:?}", result.risk_assessment);
}
```

### Trace Fund Flows

```rust
let flows = engine.trace_fund_flows(
    "source_wallet",
    "target_wallet"
).await.unwrap();

for flow in flows {
    println!("{} -> {} ({})", 
        flow.from_wallet, 
        flow.to_wallet, 
        flow.amount
    );
}
```

## Development Roadmap

- [ ] RPC integration with Solana blockchain
- [ ] SQLx database integration for caching
- [ ] REST API server
- [ ] WebAssembly compilation for browser-based analysis
- [ ] Machine learning models for pattern detection
- [ ] Visualization dashboard
- [ ] Multi-chain support (Ethereum, etc.)
- [ ] Real-time monitoring system
- [ ] Advanced graph algorithms for wallet clustering
- [ ] Temporal analysis enhancements

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
