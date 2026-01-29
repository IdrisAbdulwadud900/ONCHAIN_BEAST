#!/bin/bash
# OnChain Beast - Easy Setup & Launch Script
# For macOS - Sets up and runs the complete system

set -e

echo "🚀 OnChain Beast - One-Click Setup & Launch"
echo "==========================================="
echo ""

PROJECT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
cd "$PROJECT_DIR"

# Color codes
RED='\033[0;31m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Function to print colored output
log_info() {
    echo -e "${BLUE}ℹ️  $1${NC}"
}

log_success() {
    echo -e "${GREEN}✅ $1${NC}"
}

log_warning() {
    echo -e "${YELLOW}⚠️  $1${NC}"
}

log_error() {
    echo -e "${RED}❌ $1${NC}"
}

# Check prerequisites
log_info "Checking prerequisites..."

if ! command -v brew &> /dev/null; then
    log_error "Homebrew not found. Please install from https://brew.sh"
    exit 1
fi
log_success "Homebrew found"

if ! command -v cargo &> /dev/null; then
    log_error "Rust not found. Please install from https://rustup.rs"
    exit 1
fi
log_success "Rust found: $(rustc --version)"

# Install PostgreSQL if needed
if ! command -v psql &> /dev/null; then
    log_warning "PostgreSQL not found, installing..."
    brew install postgresql
else
    log_success "PostgreSQL found"
fi

# Install Redis if needed
if ! command -v redis-cli &> /dev/null; then
    log_warning "Redis not found, installing..."
    brew install redis
else
    log_success "Redis found"
fi

echo ""
log_info "Starting services..."

# Start PostgreSQL
if ! brew services list | grep -q "postgresql.*started"; then
    log_warning "Starting PostgreSQL..."
    brew services start postgresql
    sleep 2
fi
log_success "PostgreSQL running"

# Start Redis
if ! brew services list | grep -q "redis.*started"; then
    log_warning "Starting Redis..."
    brew services start redis
    sleep 2
fi
log_success "Redis running"

echo ""
log_info "Initializing database..."

# Create database and schema
if [ -f "init_db.sh" ]; then
    chmod +x init_db.sh
    ./init_db.sh > /dev/null 2>&1
else
    # Create database directly
    psql -c "CREATE DATABASE onchain_beast_personal;" 2>/dev/null || true
fi
log_success "Database initialized"

echo ""
log_info "Building application..."

# Check if binary exists and is recent
if [ -f "target/release/onchain_beast" ] && [ target/release/onchain_beast -nt Cargo.toml ]; then
    log_success "Binary already built ($(ls -lh target/release/onchain_beast | awk '{print $5}'))"
else
    log_warning "Building from source (this may take a minute)..."
    if ! cargo build --release --quiet; then
        log_error "Compilation failed. Please check the build output."
        exit 1
    fi
    log_success "Build complete"
fi

echo ""
log_info "Verifying configuration..."

if [ ! -f ".env" ]; then
    log_warning "Creating .env file..."
    cat > .env << 'EOF'
# OnChain Beast - Personal Configuration

# Solana RPC Configuration
SOLANA_RPC_ENDPOINT=https://api.mainnet-beta.solana.com
RPC_TIMEOUT_SECS=30
RPC_RETRY_ATTEMPTS=3

# Server Configuration
SERVER_HOST=127.0.0.1
SERVER_PORT=8080
METRICS_PORT=9090

# Database Configuration (PostgreSQL)
DATABASE_URL=postgresql://mac@localhost/onchain_beast_personal
DB_MAX_CONNECTIONS=20

# Redis Configuration
REDIS_URL=redis://127.0.0.1:6379
REDIS_POOL_SIZE=10

# API Configuration
RATE_LIMIT_PER_MINUTE=60
MAX_TRANSACTIONS_PER_REQUEST=100
ANALYSIS_CACHE_TTL_SECONDS=1800

# Logging
LOG_LEVEL=info
LOG_FILE=logs/onchain_beast.log

# Feature Flags
ENABLE_METRICS=true
ENABLE_ANALYSIS=true
ENABLE_CACHING=true
ENABLE_PERSISTENCE=true
EOF
fi
log_success ".env configured"

# Create logs directory
mkdir -p logs

echo ""
log_success "Setup complete!"
echo ""

# Display startup information
cat << 'EOF'
╔═══════════════════════════════════════════════════════════════╗
║           🎉 Ready to Launch OnChain Beast! 🎉               ║
╚═══════════════════════════════════════════════════════════════╝

📊 Service Information:
   • API Server: http://127.0.0.1:8080
   • Metrics: http://127.0.0.1:9090/metrics
   • Database: PostgreSQL (onchain_beast_personal)
   • Cache: Redis (127.0.0.1:6379)
   • RPC: Solana Mainnet

🧪 Quick Test:
   curl http://127.0.0.1:8080/health

📚 Available Endpoints:
   POST   /api/v1/parse/transaction     - Parse a transaction
   GET    /metadata/token/{mint}        - Get token info
   GET    /analysis/wallet/{address}    - Analyze wallet
   POST   /transfer/batch-analyze       - Batch transfers
   GET    /metrics                      - Prometheus metrics

📋 Next Steps:
   1. Start the service (see below)
   2. Test health: curl http://127.0.0.1:8080/health
   3. Check logs: tail -f logs/onchain_beast.log
   4. Stop services: brew services stop postgresql redis

EOF

# Ask user if they want to start the service now
echo ""
read -p "Start the service now? (y/n) " -n 1 -r
echo
if [[ $REPLY =~ ^[Yy]$ ]]; then
    echo ""
    log_info "Starting OnChain Beast..."
    echo ""
    exec ./target/release/onchain_beast
else
    log_info "To start later, run: ./target/release/onchain_beast"
fi
