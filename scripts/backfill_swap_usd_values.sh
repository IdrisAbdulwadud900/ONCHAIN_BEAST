#!/bin/bash

# Backfill USD Values for Existing Swaps
# Fetches historical prices and updates swap_events table

set -e

echo "🔄 Swap USD Value Backfill Script"
echo "=================================="
echo ""

# Configuration
DB_NAME="${DATABASE_NAME:-onchain_beast_personal}"
BATCH_SIZE="${BATCH_SIZE:-50}"
MAX_WORKERS="${MAX_WORKERS:-5}"

# Colors
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m'

# Count swaps needing enrichment
echo -e "${BLUE}Checking database...${NC}"
TOTAL_SWAPS=$(psql -d $DB_NAME -t -c "SELECT COUNT(*) FROM swap_events WHERE price_usd_in IS NULL;")
TOTAL_SWAPS=$(echo $TOTAL_SWAPS | xargs) # trim whitespace

echo "Total swaps needing enrichment: $TOTAL_SWAPS"
echo ""

if [ "$TOTAL_SWAPS" -eq 0 ]; then
    echo -e "${GREEN}✅ All swaps already have USD values!${NC}"
    exit 0
fi

# Estimate time
BATCHES=$(( ($TOTAL_SWAPS + $BATCH_SIZE - 1) / $BATCH_SIZE ))
EST_MINUTES=$(( $BATCHES / 2 ))
echo "Estimated time: ~$EST_MINUTES minutes (batches of $BATCH_SIZE)"
echo ""

read -p "Continue with backfill? (y/n) " -n 1 -r
echo
if [[ ! $REPLY =~ ^[Yy]$ ]]; then
    echo "Cancelled."
    exit 0
fi

echo ""
echo -e "${BLUE}Starting backfill...${NC}"
echo ""

# Backfill function
backfill_batch() {
    local offset=$1
    local batch=$2
    
    # Get batch of swaps
    psql -d $DB_NAME -t -c "
        SELECT 
            signature,
            token_in,
            token_out,
            amount_in,
            amount_out,
            block_time
        FROM swap_events
        WHERE price_usd_in IS NULL
        ORDER BY block_time DESC
        LIMIT $BATCH_SIZE OFFSET $offset;
    " | while IFS='|' read -r sig token_in token_out amt_in amt_out block_time; do
        # Trim whitespace
        sig=$(echo $sig | xargs)
        token_in=$(echo $token_in | xargs)
        token_out=$(echo $token_out | xargs)
        amt_in=$(echo $amt_in | xargs)
        amt_out=$(echo $amt_out | xargs)
        block_time=$(echo $block_time | xargs)
        
        if [ -z "$sig" ]; then
            continue
        fi
        
        # Fetch prices from API
        price_in=$(curl -s "http://localhost:8080/api/v1/price/$token_in" | jq -r '.price_usd // 0')
        price_out=$(curl -s "http://localhost:8080/api/v1/price/$token_out" | jq -r '.price_usd // 0')
        
        # Calculate USD values
        value_in=$(echo "$amt_in * $price_in" | bc -l)
        value_out=$(echo "$amt_out * $price_out" | bc -l)
        pnl=$(echo "$value_out - $value_in" | bc -l)
        
        # Update database
        psql -d $DB_NAME -c "
            UPDATE swap_events
            SET price_usd_in = $price_in,
                price_usd_out = $price_out,
                value_usd_in = $value_in,
                value_usd_out = $value_out,
                pnl_usd = $pnl
            WHERE signature = '$sig';
        " > /dev/null
        
        echo -e "${GREEN}✓${NC} Updated swap: ${sig:0:8}... (PnL: \$$pnl)"
    done
    
    echo -e "${BLUE}Batch $batch complete${NC}"
}

# Process batches
processed=0
for (( batch=1; batch<=$BATCHES; batch++ )); do
    offset=$(( ($batch - 1) * $BATCH_SIZE ))
    
    echo -e "${YELLOW}Processing batch $batch/$BATCHES (offset: $offset)${NC}"
    backfill_batch $offset $batch
    
    processed=$(( $processed + $BATCH_SIZE ))
    if [ $processed -gt $TOTAL_SWAPS ]; then
        processed=$TOTAL_SWAPS
    fi
    
    echo "Progress: $processed/$TOTAL_SWAPS swaps"
    echo ""
    
    # Rate limiting (don't overwhelm Jupiter API)
    sleep 2
done

# Verify results
REMAINING=$(psql -d $DB_NAME -t -c "SELECT COUNT(*) FROM swap_events WHERE price_usd_in IS NULL;")
REMAINING=$(echo $REMAINING | xargs)

echo ""
echo "=================================="
echo -e "${GREEN}✅ Backfill Complete!${NC}"
echo ""
echo "Statistics:"
echo "  - Total processed: $TOTAL_SWAPS swaps"
echo "  - Remaining (null): $REMAINING"
echo "  - Batches: $BATCHES"
echo ""

if [ "$REMAINING" -eq 0 ]; then
    echo -e "${GREEN}🎉 All swaps now have USD values!${NC}"
else
    echo -e "${YELLOW}⚠️  Some swaps still missing USD values (may be invalid tokens)${NC}"
fi

# Show PnL summary
echo ""
echo "PnL Summary:"
psql -d $DB_NAME -c "
    SELECT 
        COUNT(*) as total_swaps,
        ROUND(SUM(CASE WHEN pnl_usd > 0 THEN pnl_usd ELSE 0 END)::numeric, 2) as total_profit,
        ROUND(SUM(CASE WHEN pnl_usd < 0 THEN pnl_usd ELSE 0 END)::numeric, 2) as total_loss,
        ROUND(SUM(pnl_usd)::numeric, 2) as net_pnl
    FROM swap_events
    WHERE pnl_usd IS NOT NULL;
"

echo ""
echo -e "${BLUE}Next steps:${NC}"
echo "  1. Enable auto-enrichment for new swaps"
echo "  2. Test claim verification endpoints"
echo "  3. Run analytics queries"
