#!/usr/bin/env bash
# E2E Test: Indexer API Integration
# Tests full cycle with TypeScript indexer and REST API for UTXO selection

set -e

# Colors
RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; BLUE='\033[0;34m'; NC='\033[0m'

# Setup
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
INDEXER_DIR="$PROJECT_ROOT/tools/indexer"
MOJAVE_RPC_URL="http://127.0.0.1:8545"
API_PORT=3000
INDEXER_PID=""

echo -e "${BLUE}=== E2E Test: Indexer API Integration ===${NC}\n"

# Cleanup function
cleanup() {
    if [ -n "$INDEXER_PID" ] && kill -0 $INDEXER_PID 2>/dev/null; then
        echo -e "\n${YELLOW}Stopping indexer (PID: $INDEXER_PID)...${NC}"
        kill $INDEXER_PID 2>/dev/null || true
        wait $INDEXER_PID 2>/dev/null || true
    fi
    [ -f "$INDEXER_DIR/indexer.pid" ] && rm "$INDEXER_DIR/indexer.pid"
}
trap cleanup EXIT

#==========================================
# STEP 1: Run Batch Test Until Deposit
#==========================================
echo -e "${YELLOW}[1/5] Running batch test up to deposit completion...${NC}"
echo "  This will deploy contracts and create a deposit transaction"

export STOP_AT_STEP=10
timeout 300 "$PROJECT_ROOT/script/e2e/e2e_batch_finalization.sh" 2>&1 | tee integration_test.log || {
    CODE=$?
    [ $CODE -eq 124 ] && { echo -e "${RED}✗ Test timed out${NC}"; exit 1; }
}
unset STOP_AT_STEP

# Extract contract addresses from log
BRIDGE=$(grep "BridgeGateway: 0x" integration_test.log | tail -1 | awk '{print $2}')
WBTC=$(grep "WBTC: 0x" integration_test.log | tail -1 | awk '{print $2}')
RELAY=$(grep "BtcRelay: 0x" integration_test.log | tail -1 | awk '{print $2}')
DEPOSIT_TXID=$(grep "Deposit TXID:" integration_test.log | tail -1 | awk '{print $3}')

[ -z "$BRIDGE" ] && { echo -e "${RED}✗ Failed to extract contract addresses${NC}"; exit 1; }

echo -e "\n${GREEN}✓ Contracts deployed and deposit completed${NC}"
echo "  Bridge: $BRIDGE"
echo "  WBTC: $WBTC"
echo "  Deposit TXID: $DEPOSIT_TXID"

#==========================================
# STEP 2: Start UTXO Indexer
#==========================================
echo -e "${YELLOW}[2/5] Starting UTXO indexer...${NC}"

# Configure indexer
cat > "$INDEXER_DIR/.env" <<EOF
BRIDGE_ADDRESS=$BRIDGE
PROVIDER_URL=$MOJAVE_RPC_URL
API_PORT=$API_PORT
LOG_LEVEL=info
EOF

# Install dependencies if needed
if [ ! -d "$INDEXER_DIR/node_modules" ]; then
    echo "  Installing dependencies..."
    cd "$INDEXER_DIR" && npm install --silent && cd "$PROJECT_ROOT"
fi

# Start indexer
cd "$INDEXER_DIR"
npm run dev > "$PROJECT_ROOT/indexer_integration.log" 2>&1 &
INDEXER_PID=$!
echo $INDEXER_PID > indexer.pid
cd "$PROJECT_ROOT"

echo "  Indexer started (PID: $INDEXER_PID)"

# Wait for indexer to be ready
echo "  Waiting for indexer..."
for i in {1..30}; do
    if curl -s http://localhost:$API_PORT/health >/dev/null 2>&1; then
        echo -e "${GREEN}✓ Indexer ready${NC}\n"
        break
    fi
    [ $i -eq 30 ] && {
        echo -e "${RED}✗ Indexer failed to start${NC}"
        cat "$PROJECT_ROOT/indexer_integration.log" | tail -20
        exit 1
    }
    sleep 1
done

# Give indexer time to sync
sleep 5

#==========================================
# STEP 3: Query Indexer API
#==========================================
echo -e "${YELLOW}[3/5] Querying UTXO indexer API...${NC}"

echo -e "${BLUE}Vault Statistics:${NC}"
curl -s http://localhost:$API_PORT/stats | jq '.'

echo -e "\n${BLUE}Available UTXOs:${NC}"
UTXOS=$(curl -s http://localhost:$API_PORT/utxos)
echo "$UTXOS" | jq '.'

UTXO_COUNT=$(echo "$UTXOS" | jq '.count')
echo -e "\n${GREEN}✓ Found $UTXO_COUNT UTXO(s)${NC}\n"

[ "$UTXO_COUNT" -eq 0 ] && {
    echo -e "${RED}✗ No UTXOs found${NC}"
    cat indexer_integration.log | tail -30
    exit 1
}

#==========================================
# STEP 4: Select UTXOs via API
#==========================================
echo -e "${YELLOW}[4/5] Selecting UTXOs for withdrawal (25000 sats)...${NC}"

SELECTED=$(curl -s -X POST http://localhost:$API_PORT/utxos/select \
    -H "Content-Type: application/json" \
    -d '{"amount": "25000", "policy": "LARGEST_FIRST"}')

echo "$SELECTED" | jq '.'

COUNT=$(echo "$SELECTED" | jq '.count')
[ "$COUNT" -eq 0 ] && { echo -e "${RED}✗ No UTXOs selected${NC}"; exit 1; }

echo -e "\n${GREEN}✓ Selected $COUNT UTXO(s)${NC}"

# Extract UTXO details
UTXO_ID=$(echo "$SELECTED" | jq -r '.selected[0].utxoId')
UTXO_TXID=$(echo "$SELECTED" | jq -r '.selected[0].txid')
UTXO_VOUT=$(echo "$SELECTED" | jq -r '.selected[0].vout')
UTXO_AMOUNT=$(echo "$SELECTED" | jq -r '.selected[0].amount')

echo "  UTXO ID: $UTXO_ID"
echo "  TXID: $UTXO_TXID"
echo "  VOUT: $UTXO_VOUT"
echo "  Amount: $UTXO_AMOUNT sats"

#==========================================
# STEP 5: Request and Finalize Withdrawal
#==========================================
echo -e "${YELLOW}[5/5] Requesting withdrawal with API-selected UTXO...${NC}"

# Load accounts
[ -f "$PROJECT_ROOT/.env.e2e" ] && source "$PROJECT_ROOT/.env.e2e"

OWNERS_KEY="0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80"
OWNERS_ADDR="${OWNERS_ADDRESS:-0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266}"
DEPLOYER_KEY="0xc97833ebdbc5d3b280eaee0c826f2bd3b5959fb902d60a167d75a035c694f282"

# Generate withdrawal address
WITHDRAW_ADDR=$(bitcoin-cli -regtest getnewaddress "" "bech32")
WITHDRAW_SPK=$(bitcoin-cli -regtest getaddressinfo "$WITHDRAW_ADDR" | jq -r '.scriptPubKey')

echo "  Withdrawal: 25000 sats → $WITHDRAW_ADDR"

# Request withdrawal with API-selected UTXO

export RECIPIENT_KEY="$OWNERS_KEY"
export BRIDGE_ADDRESS="$BRIDGE"
export WBTC_ADDRESS="$WBTC"
export RECIPIENT="$OWNERS_ADDR"
export WITHDRAW_AMOUNT="25000"
export WITHDRAW_DEST_SPK="0x$WITHDRAW_SPK"
export UTXO_ID_0="$UTXO_ID"
export UTXO_TXID_0="$UTXO_TXID"
export UTXO_VOUT_0="$UTXO_VOUT"
export UTXO_AMOUNT_0="$UTXO_AMOUNT"

(cd "$PROJECT_ROOT" && forge script script/withdrawal/RequestWithdrawalWithUtxoIds.s.sol:RequestWithdrawalWithUtxoIds \
    --broadcast --rpc-url "$MOJAVE_RPC_URL" --legacy) >> integration_test.log 2>&1

# Extract Withdrawal ID from WithdrawalInitiated event
sleep 1  # Wait for event to be indexed

# topics[0] = event signature, topics[1] = wid (first indexed parameter)
EVENT_SIG=$(cast sig-event "WithdrawalInitiated(bytes32,address,uint32,uint64,bytes32,bytes)")
WID=$(cast logs --address "$BRIDGE" --from-block latest \
    "WithdrawalInitiated(bytes32,address,uint32,uint64,bytes32,bytes)" \
    --rpc-url "$MOJAVE_RPC_URL" | grep "$EVENT_SIG" -A 1 | tail -1 | tr -d '[:space:]')

if [ -z "$WID" ] || [ "$WID" = "topics:" ]; then
    echo -e "${RED}✗ No Withdrawal ID found${NC}"
    exit 1
fi

echo -e "${GREEN}✓ Withdrawal requested${NC}"
echo "  Withdrawal ID: $WID"

# Generate operator signatures and finalize (using FinalizeWithdrawal script)
echo -e "\n  Finalizing withdrawal..."

export WID="$WID"
export BRIDGE_ADDRESS="$BRIDGE"
export WBTC_ADDRESS="$WBTC"
export RECIPIENT="$OWNERS_ADDR"
export PRIVATE_KEY="$DEPLOYER_KEY"

FINALIZE_OUTPUT=$(cd "$PROJECT_ROOT" && forge script script/withdrawal/FinalizeWithdrawal.s.sol:FinalizeWithdrawal \
    --broadcast --rpc-url "$MOJAVE_RPC_URL" --legacy 2>&1)

if echo "$FINALIZE_OUTPUT" | grep -q "\[SUCCESS\]"; then
    echo -e "${GREEN}✓ Withdrawal finalized${NC}"
else
    echo -e "${RED}✗ Withdrawal finalization failed${NC}"
    echo "$FINALIZE_OUTPUT" | tail -20
    exit 1
fi

# Verify UTXO marked as spent
SPENT=$(cast call "$BRIDGE" "utxoSpent(bytes32)(bool)" "$UTXO_ID" --rpc-url "$MOJAVE_RPC_URL")
echo "  UTXO spent status: $SPENT"

# Check final balance
FINAL_BAL=$(cast call "$WBTC" "balanceOf(address)(uint256)" "$OWNERS_ADDR" --rpc-url "$MOJAVE_RPC_URL")

# Query API for updated state
sleep 2
echo -e "\n${BLUE}Final API State:${NC}"
curl -s http://localhost:$API_PORT/utxos | jq '.'

echo -e "\n${GREEN}============================================${NC}"
echo -e "${GREEN}✓ E2E Test Complete (Indexer API)${NC}"
echo -e "${GREEN}============================================${NC}"
echo -e "  Deposit: 50000 sats → wBTC minted"
echo -e "  Indexer: Synced UTXOs via REST API"
echo -e "  Withdrawal: 25000 sats using API-selected UTXO"
echo -e "  Final balance: $FINAL_BAL (25000 remaining)"
echo -e "\nLogs:"
echo -e "  - integration_test.log (full cycle)"
echo -e "  - indexer_integration.log (indexer output)"
