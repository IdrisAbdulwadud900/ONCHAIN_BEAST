#!/bin/bash
cd /Users/mac/Downloads/onchain_beast
git add -A
git commit -m "feat: Phase 2 - Enhanced transaction parsing with SOL and token transfer extraction

- Implement EnhancedTransactionParser with 850 lines of production code
- Extract actual SOL transfers from system program instructions
- Extract token transfers from SPL Token program (transfer and transferChecked)
- Process inner instructions for complex DEX transactions
- Track balance changes for all accounts
- Calculate human-readable amounts (lamports to SOL, token amounts with decimals)
- Update RPC client to use jsonParsed encoding for readable instructions
- Enhance API endpoints to expose SOL and token transfer data
- Add comprehensive transaction summary endpoint

New capabilities:
- SOL transfer extraction with from/to/amount
- Token transfer detection with mint/decimals/amounts
- Inner instruction processing (DEX swaps)
- Balance change tracking
- Program identification (20+ programs)
- Transaction classification

Performance: Full fund flow analysis now available
Files: +850 lines (enhanced_parser.rs), 4 files modified"

git push origin master
echo "✅ Phase 2 committed and pushed to GitHub!"
