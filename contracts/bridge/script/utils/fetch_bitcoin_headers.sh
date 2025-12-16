#!/bin/bash
#
# Fetch Bitcoin Headers and Prepare for Submission
#
# This script:
# 1. Fetches Bitcoin block headers from regtest
# 2. Mines valid blocks if needed
# 3. Prepares headers for submission to BtcRelay
#

set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

echo -e "${BLUE}========================================${NC}"
echo -e "${BLUE}Fetch Bitcoin Headers for BtcRelay${NC}"
echo -e "${BLUE}========================================${NC}"
echo ""

# Check Bitcoin Core
if ! command -v bitcoin-cli &> /dev/null; then
    echo -e "${RED}Error: bitcoin-cli not found${NC}"
    exit 1
fi

# Check if regtest is running
if ! bitcoin-cli -regtest getblockchaininfo &> /dev/null; then
    echo -e "${RED}Error: Bitcoin regtest not running${NC}"
    echo "Start with: bitcoind -regtest -daemon -txindex"
    exit 1
fi

# Get current block count
BLOCK_COUNT=$(bitcoin-cli -regtest getblockcount)
echo "Current block height: $BLOCK_COUNT"
echo ""

# Ensure we have enough blocks (at least 10 after genesis)
MIN_BLOCKS=10
if [ "$BLOCK_COUNT" -lt "$MIN_BLOCKS" ]; then
    echo -e "${YELLOW}Generating blocks...${NC}"
    NEEDED=$((MIN_BLOCKS - BLOCK_COUNT))
    ADDR=$(bitcoin-cli -regtest getnewaddress)
    bitcoin-cli -regtest generatetoaddress "$NEEDED" "$ADDR" > /dev/null
    BLOCK_COUNT=$(bitcoin-cli -regtest getblockcount)
    echo -e "${GREEN}✓ Generated $NEEDED blocks (now at $BLOCK_COUNT)${NC}"
    echo ""
fi

# Determine which blocks to submit
# Genesis (block 0) is already in the contract
# We'll submit blocks 1 through N
START_HEIGHT=1
END_HEIGHT=${1:-7}  # Default: submit blocks 1-7 (total 7 blocks)

if [ "$END_HEIGHT" -gt "$BLOCK_COUNT" ]; then
    echo -e "${RED}Error: Not enough blocks${NC}"
    echo "Requested up to block $END_HEIGHT but only have $BLOCK_COUNT"
    exit 1
fi

HEADER_COUNT=$((END_HEIGHT - START_HEIGHT + 1))

echo "Fetching headers from block $START_HEIGHT to $END_HEIGHT"
echo "Total headers: $HEADER_COUNT"
echo ""

# Create temporary env file for headers
HEADERS_ENV="$PROJECT_ROOT/.env.headers"
rm -f "$HEADERS_ENV"

echo "HEADER_COUNT=$HEADER_COUNT" >> "$HEADERS_ENV"
echo "" >> "$HEADERS_ENV"

# Fetch each header
for (( height=$START_HEIGHT; height<=$END_HEIGHT; height++ )); do
    index=$((height - START_HEIGHT + 1))
    
    # Get block hash
    BLOCK_HASH=$(bitcoin-cli -regtest getblockhash $height)
    
    # Get block header (hex)
    HEADER=$(bitcoin-cli -regtest getblockheader "$BLOCK_HASH" false)
    
    # Verify header length (should be 160 hex chars = 80 bytes)
    HEADER_LEN=${#HEADER}
    if [ "$HEADER_LEN" -ne 160 ]; then
        echo -e "${RED}Error: Invalid header length for block $height${NC}"
        echo "Expected 160 chars, got $HEADER_LEN"
        exit 1
    fi
    
    echo "Block $height: ${HEADER:0:16}...${HEADER: -16}"
    
    # Add to env file
    echo "HEADER_$index=0x$HEADER" >> "$HEADERS_ENV"
    echo "HEIGHT_$index=$height" >> "$HEADERS_ENV"
done

echo ""
echo -e "${GREEN}✓ Fetched $HEADER_COUNT headers${NC}"
echo ""
echo "Headers saved to: $HEADERS_ENV"
echo ""
echo "To submit headers, run:"
echo "  source $HEADERS_ENV"
echo "  forge script script/deposit/SubmitBitcoinHeaders.s.sol:SubmitBitcoinHeaders --broadcast --rpc-url \$MOJAVE_RPC_URL --legacy"
echo ""
