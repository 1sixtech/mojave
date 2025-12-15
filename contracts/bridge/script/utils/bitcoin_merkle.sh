#!/usr/bin/env bash

# Bitcoin Merkle Proof Calculator
# Calculates merkle branch for a transaction in a block

set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

# Default network
NETWORK="regtest"

# Usage
usage() {
    echo "Usage: $0 <txid> [--network <net>]"
    echo ""
    echo "Arguments:"
    echo "  <txid>             Transaction ID to calculate merkle proof for"
    echo ""
    echo "Options:"
    echo "  --network <net>    Bitcoin network (regtest/testnet/mainnet, default: regtest)"
    echo "  --help            Show this help message"
    echo ""
    echo "Example:"
    echo "  $0 abc123...def456 --network regtest"
    exit 1
}

# Parse arguments
TXID=""
while [[ $# -gt 0 ]]; do
    case $1 in
        --network)
            NETWORK="$2"
            shift 2
            ;;
        --help)
            usage
            ;;
        *)
            if [ -z "$TXID" ]; then
                TXID="$1"
            else
                echo -e "${RED}Unknown option: $1${NC}"
                usage
            fi
            shift
            ;;
    esac
done

if [ -z "$TXID" ]; then
    echo -e "${RED}Error: TXID is required${NC}"
    usage
fi

# Set bitcoin-cli network flag
if [ "$NETWORK" = "testnet" ]; then
    BTC_CLI=(bitcoin-cli -testnet)
elif [ "$NETWORK" = "regtest" ]; then
    BTC_CLI=(bitcoin-cli -regtest)
else
    BTC_CLI=(bitcoin-cli)
fi

echo -e "${GREEN}========================================${NC}"
echo -e "${GREEN}Bitcoin Merkle Proof Calculator${NC}"
echo -e "${GREEN}========================================${NC}"
echo ""
echo "Network: $NETWORK"
echo "TXID: $TXID"
echo ""

# Step 1: Get transaction details to find block
echo -e "${YELLOW}[1/4] Finding block...${NC}"
TX_INFO=$("${BTC_CLI[@]}" getrawtransaction "$TXID" true 2>/dev/null || echo "error")
if [ "$TX_INFO" = "error" ]; then
    echo -e "${RED}Error: Transaction not found${NC}"
    echo "Make sure:"
    echo "  1. bitcoind is running with -txindex"
    echo "  2. Transaction is confirmed (in a block)"
    exit 1
fi

BLOCK_HASH=$(echo "$TX_INFO" | jq -r '.blockhash')
if [ "$BLOCK_HASH" = "null" ] || [ -z "$BLOCK_HASH" ]; then
    echo -e "${RED}Error: Transaction not yet in a block${NC}"
    echo "Wait for confirmation or generate blocks in regtest"
    exit 1
fi

echo "Block hash: $BLOCK_HASH"
echo ""

# Step 2: Get full block with all transactions
echo -e "${YELLOW}[2/4] Fetching block data...${NC}"
BLOCK_DATA=$("${BTC_CLI[@]}" getblock "$BLOCK_HASH" 2)
if [ -z "$BLOCK_DATA" ]; then
    echo -e "${RED}Error: Failed to fetch block${NC}"
    exit 1
fi

# Step 3: Find transaction index in block
echo -e "${YELLOW}[3/4] Finding transaction index...${NC}"
TX_INDEX=-1
TX_COUNT=$(echo "$BLOCK_DATA" | jq '.tx | length')

for ((i=0; i<TX_COUNT; i++)); do
    CURRENT_TXID=$(echo "$BLOCK_DATA" | jq -r ".tx[$i].txid")
    if [ "$CURRENT_TXID" = "$TXID" ]; then
        TX_INDEX=$i
        break
    fi
done

if [ $TX_INDEX -eq -1 ]; then
    echo -e "${RED}Error: Transaction not found in block${NC}"
    exit 1
fi

echo "Transaction index: $TX_INDEX"
echo "Total transactions: $TX_COUNT"
echo ""

# Step 4: Calculate merkle proof
echo -e "${YELLOW}[4/4] Calculating merkle proof...${NC}"

# Extract all transaction hashes
declare -a TX_HASHES
for ((i=0; i<TX_COUNT; i++)); do
    HASH=$(echo "$BLOCK_DATA" | jq -r ".tx[$i].txid")
    # Convert to little-endian bytes (Bitcoin internal format)
    TX_HASHES[$i]=$HASH
done

# Calculate merkle branch directly (avoiding nameref for bash 3.2 compatibility)
declare -a MERKLE_SIBLINGS=()
local_index=$TX_INDEX
local_count=$TX_COUNT

# Build merkle tree level by level
while [ $local_count -gt 1 ]; do
    # Find sibling at this level
    if [ $((local_index % 2)) -eq 0 ]; then
        # Left node - sibling is right
        if [ $((local_index + 1)) -lt $local_count ]; then
            MERKLE_SIBLINGS+=("${TX_HASHES[$((local_index + 1))]}")
        else
            # Duplicate if odd number of nodes
            MERKLE_SIBLINGS+=("${TX_HASHES[$local_index]}")
        fi
    else
        # Right node - sibling is left
        MERKLE_SIBLINGS+=("${TX_HASHES[$((local_index - 1))]}")
    fi
    
    # Move to next level
    local_index=$((local_index / 2))
    local_count=$(( (local_count + 1) / 2 ))
done

echo "Merkle branch length: ${#MERKLE_SIBLINGS[@]}"
echo ""

# Build merkle proof hex (concatenated 32-byte hashes)
MERKLE_PROOF=""
for sibling in "${MERKLE_SIBLINGS[@]}"; do
    # Remove 0x prefix if present and reverse bytes (little-endian)
    sibling="${sibling#0x}"
    # Reverse byte order for Bitcoin internal format
    reversed=""
    for ((i=${#sibling}-2; i>=0; i-=2)); do
        reversed+="${sibling:$i:2}"
    done
    MERKLE_PROOF+="$reversed"
done

# Get block header
BLOCK_HEADER=$("${BTC_CLI[@]}" getblockheader "$BLOCK_HASH" false)

echo -e "${GREEN}========================================${NC}"
echo -e "${GREEN}SUCCESS!${NC}"
echo -e "${GREEN}========================================${NC}"
echo ""

# Save to .env.merkle file for automation
# This script is in script/utils/, so go up 2 levels to reach project root
PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cat > "$PROJECT_ROOT/.env.merkle" <<EOF
BITCOIN_DEPOSIT_TXID=0x$TXID
BITCOIN_BLOCK_HASH=0x$BLOCK_HASH
BITCOIN_RAW_TX=0x$(echo "$TX_INFO" | jq -r '.hex')
BITCOIN_BLOCK_HEADER=0x$BLOCK_HEADER
BITCOIN_MERKLE_INDEX=$TX_INDEX
BITCOIN_MERKLE_PROOF=0x$MERKLE_PROOF
EOF

echo "Export these variables for Step3:"
echo ""
echo -e "${YELLOW}export BITCOIN_DEPOSIT_TXID=0x$TXID${NC}"
echo -e "${YELLOW}export BITCOIN_BLOCK_HASH=0x$BLOCK_HASH${NC}"
echo -e "${YELLOW}export BITCOIN_RAW_TX=0x$(echo "$TX_INFO" | jq -r '.hex')${NC}"
echo -e "${YELLOW}export BITCOIN_BLOCK_HEADER=0x$BLOCK_HEADER${NC}"
echo -e "${YELLOW}export BITCOIN_MERKLE_INDEX=$TX_INDEX${NC}"
echo -e "${YELLOW}export BITCOIN_MERKLE_PROOF=0x$MERKLE_PROOF${NC}"
echo ""
echo "Merkle branch details:"
for ((i=0; i<${#MERKLE_SIBLINGS[@]}; i++)); do
    echo "  [$i] ${MERKLE_SIBLINGS[$i]}"
done
echo ""
echo "(Also saved to .env.merkle)"
echo ""
