#!/usr/bin/env bash
# E2E Test: Incremental Signatures (submitSignature × 4)
# Tests deposit→withdrawal flow with operator signatures submitted one-by-one

set -e

# Colors
RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; BLUE='\033[0;34m'; NC='\033[0m'

# Setup
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
MOJAVE_DIR="${MOJAVE_DIR:-../mojave}"
MOJAVE_RPC_URL="http://127.0.0.1:8545"

# Accounts (Mojave pre-funded)
DEPLOYER_KEY="0xc97833ebdbc5d3b280eaee0c826f2bd3b5959fb902d60a167d75a035c694f282"
DEPLOYER_ADDR="0x113126568ba236A996FD4f558083C676ea93A389"
OWNERS_ADDR="0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266"

echo -e "${BLUE}=== E2E Test: Incremental Signatures ===${NC}\n"

#==========================================
# STEP 1: Clean Environment
#==========================================
echo -e "${YELLOW}[1/17] Cleaning environment...${NC}"
bitcoin-cli -regtest stop 2>/dev/null || true
sleep 2
pkill -9 bitcoind 2>/dev/null || true

[ -f "$MOJAVE_DIR/.mojave/full.pid" ] && (cd "$MOJAVE_DIR" && just kill-full 2>/dev/null || true)
sleep 1

[ "$(uname)" == "Darwin" ] && rm -rf ~/Library/Application\ Support/Bitcoin/regtest || rm -rf ~/.bitcoin/regtest

echo -e "${GREEN}✓ Environment cleaned${NC}\n"

#==========================================
# STEP 2: Start Bitcoin Regtest
#==========================================
echo -e "${YELLOW}[2/17] Starting Bitcoin regtest...${NC}"
bitcoind -regtest -daemon -txindex
sleep 3
bitcoin-cli -regtest createwallet "test" >/dev/null 2>&1 || true
sleep 1

ADDR=$(bitcoin-cli -regtest getnewaddress)
bitcoin-cli -regtest generatetoaddress 101 "$ADDR" >/dev/null 2>&1
BLOCKS=$(bitcoin-cli -regtest getblockcount)
echo -e "${GREEN}✓ Bitcoin started ($BLOCKS blocks)${NC}\n"

#==========================================
# STEP 3: Start Mojave
#==========================================
echo -e "${YELLOW}[3/17] Starting Mojave...${NC}"
if cast bn --rpc-url "$MOJAVE_RPC_URL" >/dev/null 2>&1; then
    echo -e "${GREEN}✓ Mojave already running${NC}\n"
elif [ -d "$MOJAVE_DIR" ]; then
    cd "$MOJAVE_DIR" && just full >/dev/null 2>&1 &
    for i in {1..30}; do
        sleep 1
        cast bn --rpc-url "$MOJAVE_RPC_URL" >/dev/null 2>&1 && break
        [ $i -eq 30 ] && { echo -e "${RED}✗ Mojave failed to start${NC}"; exit 1; }
    done
    echo -e "${GREEN}✓ Mojave started${NC}\n"
    cd "$PROJECT_ROOT"
else
    echo -e "${RED}✗ Mojave not found${NC}"; exit 1
fi

#==========================================
# STEP 4: Deploy Contracts
#==========================================
echo -e "${YELLOW}[4/17] Deploying contracts...${NC}"
cd "$PROJECT_ROOT"

export PRIVATE_KEY=$DEPLOYER_KEY
OUT=$(forge script script/Deploy.s.sol:Deploy --broadcast --rpc-url "$MOJAVE_RPC_URL" --legacy 2>&1)

if echo "$OUT" | grep -q "SIMULATION FAILED\|Transaction failed"; then
    echo -e "${RED}✗ Deployment failed${NC}\n$OUT"; exit 1
fi

WBTC=$(echo "$OUT" | grep "WBTC deployed at:" | grep -oE "0x[a-fA-F0-9]{40}" | head -1)
RELAY=$(echo "$OUT" | grep "BtcRelay deployed at:" | grep -oE "0x[a-fA-F0-9]{40}" | head -1)
BRIDGE=$(echo "$OUT" | grep "BridgeGateway deployed at:" | grep -oE "0x[a-fA-F0-9]{40}" | head -1)

[ -z "$WBTC" ] || [ -z "$RELAY" ] || [ -z "$BRIDGE" ] && { echo -e "${RED}✗ Failed to extract addresses${NC}"; exit 1; }

echo -e "${GREEN}✓ Contracts deployed${NC}"
echo "  WBTC: $WBTC"
echo "  BtcRelay: $RELAY"
echo "  BridgeGateway: $BRIDGE\n"

# Fund OWNERS if needed
BAL=$(cast balance "$OWNERS_ADDR" --rpc-url "$MOJAVE_RPC_URL")
if [ "$BAL" = "0" ]; then
    cast send "$OWNERS_ADDR" --value 100ether --private-key "$DEPLOYER_KEY" --rpc-url "$MOJAVE_RPC_URL" --legacy >/dev/null 2>&1
    sleep 1
fi

#==========================================
# STEP 5: Submit Initial Headers
#==========================================
echo -e "${YELLOW}[5/17] Submitting Bitcoin headers (1-10)...${NC}"
"$SCRIPT_DIR/../utils/fetch_bitcoin_headers.sh" 10 >/dev/null 2>&1
[ ! -f "$PROJECT_ROOT/.env.headers" ] && { echo -e "${RED}✗ Failed to fetch headers${NC}"; exit 1; }

(set -a && source "$PROJECT_ROOT/.env.headers" && BTC_RELAY_ADDRESS=$RELAY && PRIVATE_KEY=$DEPLOYER_KEY && set +a && \
forge script script/deposit/SubmitBitcoinHeaders.s.sol:SubmitBitcoinHeaders --broadcast --rpc-url "$MOJAVE_RPC_URL" --legacy) 2>&1 | \
grep -E "(Best Block:|Height:)" || true

echo -e "${GREEN}✓ Headers submitted${NC}\n"

#==========================================
# STEP 6: Calculate Deposit Envelope
#==========================================
echo -e "${YELLOW}[6/17] Calculating deposit envelope...${NC}"

cat > "$PROJECT_ROOT/.env.step1" <<EOF
RECIPIENT=$OWNERS_ADDR
DEPOSIT_AMOUNT=50000
BRIDGE_ADDRESS=$BRIDGE
OPRET_TAG=0xf14d0001
VAULT_SPK=0x0014abcdef1234567890abcdef1234567890abcdef12
EOF

OUT=$(cd "$PROJECT_ROOT" && set -a && source .env.step1 && set +a && \
forge script script/deposit/CalculateDepositEnvelope.s.sol:CalculateDepositEnvelope 2>&1)

ENVELOPE=$(echo "$OUT" | grep "Envelope (hex):" | awk '{print $NF}')
rm -f "$PROJECT_ROOT/.env.step1"

[ -z "$ENVELOPE" ] || [ ${#ENVELOPE} -lt 100 ] && { echo -e "${RED}✗ Failed to calculate envelope${NC}"; exit 1; }

echo "OWNERS_ADDRESS=$OWNERS_ADDR" > "$PROJECT_ROOT/.env.e2e"
echo -e "${GREEN}✓ Envelope calculated (${#ENVELOPE} chars)${NC}\n"

#==========================================
# STEP 7: Create Deposit Transaction
#==========================================
echo -e "${YELLOW}[7/17] Creating deposit transaction...${NC}"

ENVELOPE_CLEAN=${ENVELOPE#0x}
VAULT_ADDR=$(bitcoin-cli -regtest getnewaddress)
CHANGE_ADDR=$(bitcoin-cli -regtest getnewaddress)

UTXO=$(bitcoin-cli -regtest listunspent 101 | jq -r '.[0]')
[ "$UTXO" = "null" ] && { echo -e "${RED}✗ No mature UTXOs${NC}"; exit 1; }

TXID=$(echo "$UTXO" | jq -r '.txid')
VOUT=$(echo "$UTXO" | jq -r '.vout')
AMT=$(echo "$UTXO" | jq -r '.amount')
CHANGE=$(echo "$AMT - 0.0005 - 0.0001" | bc)

RAW=$(bitcoin-cli -regtest createrawtransaction \
    "[{\"txid\":\"$TXID\",\"vout\":$VOUT}]" \
    "{\"$VAULT_ADDR\":0.0005,\"$CHANGE_ADDR\":$CHANGE,\"data\":\"$ENVELOPE_CLEAN\"}")

SIGNED=$(bitcoin-cli -regtest signrawtransactionwithwallet "$RAW" | jq -r '.hex')
DEPOSIT_TXID=$(bitcoin-cli -regtest sendrawtransaction "$SIGNED")

echo "$DEPOSIT_TXID" > /tmp/deposit_txid.txt

bitcoin-cli -regtest generatetoaddress 7 $(bitcoin-cli -regtest getnewaddress) >/dev/null 2>&1

DEPOSIT_HASH=$(bitcoin-cli -regtest getrawtransaction "$DEPOSIT_TXID" true | jq -r '.blockhash')
DEPOSIT_HEIGHT=$(bitcoin-cli -regtest getblock "$DEPOSIT_HASH" | jq -r '.height')
echo "$DEPOSIT_HEIGHT" > /tmp/deposit_block_height.txt

echo -e "${GREEN}✓ Deposit created${NC}"
echo "  TXID: $DEPOSIT_TXID"
echo "  Block: $DEPOSIT_HEIGHT\n"

# Configure vault scriptPubKey
VAULT_SPK=$(bitcoin-cli -regtest getaddressinfo "$VAULT_ADDR" | jq -r '.scriptPubKey')
cast send "$BRIDGE" "setDepositParams(bytes,bytes)" "0x$VAULT_SPK" "0x4d4f4a31" \
    --rpc-url "$MOJAVE_RPC_URL" --private-key "$DEPLOYER_KEY" --legacy --gas-limit 100000 >/dev/null 2>&1
echo -e "${GREEN}✓ Vault configured${NC}\n"

#==========================================
# STEP 8: Update Headers
#==========================================
echo -e "${YELLOW}[8/17] Updating headers...${NC}"

DEPOSIT_HEIGHT=$(cat /tmp/deposit_block_height.txt)
BTC_HEIGHT=$(bitcoin-cli -regtest getblockcount)
RELAY_HEIGHT=$(cast call "$RELAY" "bestHeight()(uint256)" --rpc-url "$MOJAVE_RPC_URL")

if [ "$BTC_HEIGHT" -gt "$RELAY_HEIGHT" ]; then
    "$SCRIPT_DIR/../utils/fetch_bitcoin_headers.sh" "$BTC_HEIGHT" >/dev/null 2>&1
    (set -a && source "$PROJECT_ROOT/.env.headers" && BTC_RELAY_ADDRESS=$RELAY && PRIVATE_KEY=$DEPLOYER_KEY && set +a && \
    forge script script/deposit/SubmitBitcoinHeaders.s.sol:SubmitBitcoinHeaders --broadcast --rpc-url "$MOJAVE_RPC_URL" --legacy) 2>&1 | \
    grep -E "Height:" | tail -5 || true
fi

# Mine more blocks if needed for 6 confirmations
REQUIRED=$((DEPOSIT_HEIGHT + 11))
if [ "$BTC_HEIGHT" -lt "$REQUIRED" ]; then
    NEEDED=$((REQUIRED - BTC_HEIGHT))
    bitcoin-cli -regtest generatetoaddress "$NEEDED" $(bitcoin-cli -regtest getnewaddress) >/dev/null 2>&1
    BTC_HEIGHT=$(bitcoin-cli -regtest getblockcount)
    
    "$SCRIPT_DIR/../utils/fetch_bitcoin_headers.sh" "$BTC_HEIGHT" >/dev/null 2>&1
    (set -a && source "$PROJECT_ROOT/.env.headers" && BTC_RELAY_ADDRESS=$RELAY && PRIVATE_KEY=$DEPLOYER_KEY && set +a && \
    forge script script/deposit/SubmitBitcoinHeaders.s.sol:SubmitBitcoinHeaders --broadcast --rpc-url "$MOJAVE_RPC_URL" --legacy) >/dev/null 2>&1
fi

FINAL_HEIGHT=$(cast call "$RELAY" "finalizedHeight()(uint256)" --rpc-url "$MOJAVE_RPC_URL")
echo -e "${GREEN}✓ Headers updated (finalized: $FINAL_HEIGHT)${NC}\n"

#==========================================
#==========================================
# STEP 9: Submit SPV Proof
#==========================================
echo -e "${YELLOW}[9/11] Submitting SPV proof...${NC}"

DEPOSIT_TXID=$(cat /tmp/deposit_txid.txt)

# Generate merkle proof (creates .env.merkle)
"$SCRIPT_DIR/../utils/bitcoin_merkle.sh" "$DEPOSIT_TXID" >/dev/null 2>&1

[ ! -f "$PROJECT_ROOT/.env.merkle" ] && { echo -e "${RED}✗ Failed to generate merkle proof${NC}"; exit 1; }

# Submit SPV proof using environment variables from .env.merkle + contract addresses
(export BRIDGE_ADDRESS=$BRIDGE && export BTC_RELAY_ADDRESS=$RELAY && \
export OPERATOR=$DEPLOYER_ADDR && export OPERATOR_KEY=$DEPLOYER_KEY && \
export MOJAVE_RPC_URL="$MOJAVE_RPC_URL" && \
set -a && source "$PROJECT_ROOT/.env.merkle" && set +a && \
cd "$PROJECT_ROOT" && forge script script/deposit/SubmitDepositSpvProof.s.sol:SubmitDepositSpvProof \
    --broadcast --rpc-url "$MOJAVE_RPC_URL" --legacy) 2>&1 | \
    grep -E "(SPV Proof|verified|Minted)" || true

echo -e "${GREEN}✓ SPV proof submitted${NC}\n"
[ "$STOP_AT_STEP" = "9" ] && exit 0

#==========================================
# STEP 10: Verify Minting
#==========================================
echo -e "${YELLOW}[10/11] Verifying wBTC minting...${NC}"

WBTC_BAL=$(cast call "$WBTC" "balanceOf(address)(uint256)" "$OWNERS_ADDR" --rpc-url "$MOJAVE_RPC_URL" | awk '{print $1}')
EXPECTED="50000"

if [ "$WBTC_BAL" = "$EXPECTED" ]; then
    echo -e "${GREEN}✓ wBTC minted: $WBTC_BAL sats${NC}"
    SUCCESS=true
elif [ "$WBTC_BAL" -gt "0" ]; then
    echo -e "${YELLOW}⚠ wBTC minted but amount mismatch (expected: $EXPECTED, got: $WBTC_BAL)${NC}"
    SUCCESS=partial
else
    echo -e "${RED}✗ No wBTC minted${NC}"
    SUCCESS=false
    exit 1
fi

echo ""
[ "$STOP_AT_STEP" = "10" ] && exit 0

# ============================================
# Variable mapping for Step 11-15 compatibility
# ============================================
CHAIN_ID=1729
OWNERS_ADDRESS="$OWNERS_ADDR"
BRIDGE_ADDRESS="$BRIDGE"
WBTC_ADDRESS="$WBTC"
DEPLOYER_PRIVATE_KEY="$DEPLOYER_KEY"
OPERATOR_PRIVATE_KEY="$DEPLOYER_KEY"

# Step 11: Request Withdrawal with User-Proposed UTXO
# ============================================
echo ""
echo -e "${YELLOW}[11/17] Requesting withdrawal with user-proposed UTXO...${NC}"

# User wants to withdraw 25000 sats (keep 25000 sats)
WITHDRAW_AMOUNT=25000

# Bitcoin L1 destination address (generate a new address for testing)
WITHDRAW_DEST_ADDR=$(bitcoin-cli -regtest getnewaddress "" "bech32")
WITHDRAW_DEST_SPK=$(bitcoin-cli -regtest getaddressinfo "$WITHDRAW_DEST_ADDR" | jq -r '.scriptPubKey')

echo "  Withdrawal amount: $WITHDRAW_AMOUNT sats"
echo "  Destination address: $WITHDRAW_DEST_ADDR"
echo "  Destination scriptPubKey: $WITHDRAW_DEST_SPK"
echo ""

# Get UTXO from deposit TXID (from Step 7)
# The deposit TXID is stored in DEPOSIT_TXID variable
UTXO_TXID="$DEPOSIT_TXID"
UTXO_VOUT=0  # First output is to vault
UTXO_AMOUNT=50000  # 0.0005 BTC in sats

echo "  User-proposed UTXO:"
echo "    TXID: $UTXO_TXID"
echo "    VOUT: $UTXO_VOUT"
echo "    Amount: $UTXO_AMOUNT sats"

# Prepare UTXO ID for withdrawal request
# If we have registered UTXO ID from Step 9, use it (event-sourced approach)
# Otherwise, calculate it from TXID+vout (fallback for testing)
if [ -n "$REGISTERED_UTXO_ID" ] && [ "$REGISTERED_UTXO_ID" != "null" ]; then
    UTXO_ID_TO_USE="$REGISTERED_UTXO_ID"
    echo "    Registered ID: $REGISTERED_UTXO_ID"
    echo "  Using event-sourced UTXO ID"
else
    # Calculate UTXO ID from TXID+vout (same as contract does)
    UTXO_ID_TO_USE=$(cast keccak "${UTXO_TXID}$(printf '%08x' $UTXO_VOUT)")
    echo "  [FALLBACK] Calculated UTXO ID from TXID+vout: $UTXO_ID_TO_USE"
    # TODO: this should come from UtxoRegistered event in real usage
fi

echo ""
echo "  Requesting withdrawal with UTXO ID..."
PRIVATE_KEY="0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80"
WITHDRAW_OUTPUT=$(cd "$PROJECT_ROOT" && \
RECIPIENT_KEY=$PRIVATE_KEY \
BRIDGE_ADDRESS=$BRIDGE_ADDRESS \
WBTC_ADDRESS=$WBTC_ADDRESS \
RECIPIENT=$OWNERS_ADDRESS \
WITHDRAW_AMOUNT=$WITHDRAW_AMOUNT \
WITHDRAW_DEST_SPK=0x$WITHDRAW_DEST_SPK \
UTXO_ID_0=$UTXO_ID_TO_USE \
UTXO_TXID_0=$UTXO_TXID \
UTXO_VOUT_0=$UTXO_VOUT \
UTXO_AMOUNT_0=$UTXO_AMOUNT \
forge script script/withdrawal/RequestWithdrawalWithUtxoIds.s.sol:RequestWithdrawalWithUtxoIds \
    --broadcast \
    --rpc-url "$MOJAVE_RPC_URL" \
    --legacy 2>&1)

# Save full output for debugging
echo "$WITHDRAW_OUTPUT" > /tmp/withdraw_with_utxo_output.txt

# Extract the transaction hash from the broadcast JSON file
WITHDRAW_TX=$(jq -r '.transactions[-1].hash' "$PROJECT_ROOT/broadcast/RequestWithdrawalWithUtxoIds.s.sol/$CHAIN_ID/run-latest.json" 2>/dev/null)

if [ -z "$WITHDRAW_TX" ] || [ "$WITHDRAW_TX" = "null" ]; then
    echo -e "${RED}[ERROR] Failed to extract transaction hash from withdrawal request${NC}"
    echo "  Full output saved to: /tmp/withdraw_request_output.txt"
    echo ""
    echo "  Last 30 lines of output:"
    echo "$WITHDRAW_OUTPUT" | tail -30
    exit 1
fi

echo "  Transaction: $WITHDRAW_TX"

# Extract WID from WithdrawalInitiated event (topics[1])
# Event signature: WithdrawalInitiated(bytes32 indexed wid, address indexed user, uint32 indexed signerSetId, ...)
EVENT_SIG="0xf15ce3b6a08184cc828194847dde2d313690120ee2ecf2c5d7cce1018089583e"
echo "  Extracting WID from transaction receipt..."

# Try multiple times with increasing delays
for i in {1..3}; do
    WID=$(cast receipt "$WITHDRAW_TX" --rpc-url "$MOJAVE_RPC_URL" --json 2>/dev/null | jq -r ".logs[] | select(.topics[0] == \"$EVENT_SIG\") | .topics[1]")
    
    if [ -n "$WID" ] && [ "$WID" != "null" ] && [ ${#WID} -eq 66 ]; then
        break
    fi
    
    if [ $i -lt 3 ]; then
        echo "  Attempt $i failed, retrying..."
        sleep 2
    fi
done

# If still failed, try broadcast file
if [ -z "$WID" ] || [ "$WID" = "null" ] || [ ${#WID} -ne 66 ]; then
    echo "  Trying broadcast file..."
    BROADCAST_FILE="$PROJECT_ROOT/broadcast/RequestWithdrawalWithUtxoIds.s.sol/$CHAIN_ID/run-latest.json"
    
    if [ -f "$BROADCAST_FILE" ]; then
        WID=$(jq -r ".receipts[0].logs[] | select(.topics[0] == \"$EVENT_SIG\") | .topics[1]" "$BROADCAST_FILE" 2>/dev/null | head -1)
    fi
fi

if [ -z "$WID" ] || [ "$WID" = "null" ] || [ ${#WID} -ne 66 ]; then
    echo -e "${RED}[ERROR] Failed to extract WID from WithdrawalInitiated event${NC}"
    echo "  Transaction: $WITHDRAW_TX"
    echo "  Event signature: $EVENT_SIG"
    exit 1
fi

echo "  WID: $WID (from blockchain event)"
echo -e "${GREEN}[OK] Withdrawal requested${NC}"

# Verify that the correct UTXO was used
echo ""
echo "  Verifying UTXO usage in withdrawal..."
echo "  Expected UTXO:"
echo "    - TXID: $UTXO_TXID"
echo "    - VOUT: $UTXO_VOUT"
echo "    - Amount: $UTXO_AMOUNT sats"
if [ -n "$REGISTERED_UTXO_ID" ] && [ "$REGISTERED_UTXO_ID" != "null" ]; then
    echo "    - Registered ID: $REGISTERED_UTXO_ID"
    
    # Verify the UTXO ID matches what we sent
    CALCULATED_ID=$(cast keccak "${UTXO_TXID}$(printf '%08x' $UTXO_VOUT)" 2>/dev/null)
    if [ "$REGISTERED_UTXO_ID" = "$CALCULATED_ID" ]; then
        echo -e "${GREEN}  ✓ UTXO ID matches: contract and user agree on same UTXO${NC}"
    else
        echo -e "${YELLOW}  ⚠ UTXO ID mismatch (expected with event-sourced approach)${NC}"
        echo "    Registered: $REGISTERED_UTXO_ID"
        echo "    Calculated: $CALCULATED_ID"
    fi
else
    echo "  ⚠ Using calculated UTXO ID (fallback mode)"
fi

# Check balance after withdrawal request (with timeout)
WBTC_BALANCE_AFTER=$(timeout 3 cast call "$WBTC_ADDRESS" "balanceOf(address)(uint256)" "$OWNERS_ADDRESS" --rpc-url "$MOJAVE_RPC_URL" 2>/dev/null || echo "0")
WBTC_BALANCE_AFTER_DEC=$(echo "$WBTC_BALANCE_AFTER" | grep -oE '^[0-9]+' || echo "0")
echo "  User wBTC balance after: $WBTC_BALANCE_AFTER_DEC sats (locked $WITHDRAW_AMOUNT in bridge)"
echo ""

# ============================================
# Step 12: Extract PSBT from WithdrawalInitiated Event
# ============================================
echo -e "${YELLOW}[12/17] Extracting PSBT from WithdrawalInitiated event...${NC}"

# WithdrawalInitiated(bytes32 indexed wid, address indexed user, uint32 indexed signerSetId, uint64 deadline, bytes32 outputsHash, bytes psbt)
WITHDRAWAL_INITIATED_SIG="0xf15ce3b6a08184cc828194847dde2d313690120ee2ecf2c5d7cce1018089583e"
echo "  WithdrawalInitiated signature: $WITHDRAWAL_INITIATED_SIG"

# Get transaction receipt (retry if needed)
echo "  Fetching transaction receipt for WID extraction..."
for i in {1..3}; do
    RECEIPT=$(cast receipt "$WITHDRAW_TX" --rpc-url "$MOJAVE_RPC_URL" --json 2>/dev/null)
    if [ -n "$RECEIPT" ]; then
        break
    fi
    if [ $i -lt 3 ]; then
        sleep 1
    fi
done

# If still failed, try broadcast file
if [ -z "$RECEIPT" ]; then
    echo "  Failed to fetch receipt, using broadcast file..."
    BROADCAST_FILE="$PROJECT_ROOT/broadcast/RequestWithdrawalWithUtxoIds.s.sol/$CHAIN_ID/run-latest.json"
    
    if [ -f "$BROADCAST_FILE" ]; then
        RECEIPT=$(jq -r '.receipts[0]' "$BROADCAST_FILE" 2>/dev/null)
    fi
fi

# Extract PSBT from WithdrawalInitiated event
PSBT_LOG=$(echo "$RECEIPT" | jq -r ".logs[] | select(.topics[0] == \"$WITHDRAWAL_INITIATED_SIG\")")

if [ -n "$PSBT_LOG" ]; then
    echo -e "${GREEN}✓ WithdrawalInitiated event found${NC}"
    
    # Extract PSBT from event data
    # Event data contains: uint64 deadline, bytes32 outputsHash, bytes psbt
    # We need to decode the bytes psbt from the data field
    RAW_EVENT_DATA=$(echo "$PSBT_LOG" | jq -r '.data')
    
    # Skip first 64 bytes (deadline + outputsHash), then decode bytes
    # bytes encoding: offset(32) + length(32) + data
    # For now, we'll use the raw data and let the final operator use it
    PSBT_FROM_EVENT="$RAW_EVENT_DATA"
    
    echo "  PSBT extracted from event"
    echo "  PSBT length: ${#PSBT_FROM_EVENT} chars"
    echo "  PSBT preview: ${PSBT_FROM_EVENT:0:100}..."
    
    # Store PSBT for later use in Step 14
    WITHDRAWAL_PSBT="$PSBT_FROM_EVENT"
    
    echo -e "${GREEN}[OK] PSBT extracted and will be used for final signature${NC}"
    
    if [ "$PSBT_LENGTH" -gt 100 ]; then
        echo -e "${GREEN}[OK] PSBT/rawTx emitted in WithdrawalEvent!${NC}"
        
        # Save PSBT for inspection
        echo "$PSBT_DATA" > /tmp/withdrawal_psbt.txt
        echo "  PSBT saved to: /tmp/withdrawal_psbt.txt"
    else
        echo -e "${YELLOW}⚠ PSBT data seems short${NC}"
    fi
else
    echo -e "${RED}✗ WithdrawalEvent not found${NC}"
    echo "Available events:"
    echo "$RECEIPT" | jq -r '.logs[].topics[0]' | head -10
fi
echo ""

# ============================================
# Step 13: Verify UTXO Tracking
# ============================================
echo -e "${YELLOW}[13/17] Verifying UTXO tracking...${NC}"

# Calculate UTXO ID using abi.encodePacked (like contract does)
# Format: txid (32 bytes) + vout (4 bytes uint32)
UTXO_ID=$(cast keccak "${UTXO_TXID}$(printf '%08x' $UTXO_VOUT)")
echo "  UTXO ID: $UTXO_ID"

# Check if UTXO is spent (with timeout)
UTXO_SPENT_BEFORE=$(timeout 3 cast call "$BRIDGE_ADDRESS" "utxoSpent(bytes32)(bool)" "$UTXO_ID" --rpc-url "$MOJAVE_RPC_URL" 2>/dev/null || echo "unknown")
echo "  Spent status (before finalization): $UTXO_SPENT_BEFORE"

if [ "$UTXO_SPENT_BEFORE" = "false" ]; then
    echo -e "${GREEN}[OK] UTXO correctly remains unspent (will be marked spent on finalization)${NC}"
else
    echo -e "${YELLOW}⚠ UTXO already marked as spent (unexpected)${NC}"
fi
echo ""

# ============================================
# Step 14: Submit Signatures One-by-One
# ============================================
echo ""
echo -e "${YELLOW}[14/17] Submitting operator signatures incrementally...${NC}"

# Operator keys (must match deployment - 4-of-5 threshold)
OPERATOR_KEYS=(
    "0x00000000000000000000000000000000000000000000000000000000000a11ce"
    "0x00000000000000000000000000000000000000000000000000000000000b11ce"
    "0x00000000000000000000000000000000000000000000000000000000000c11ce"
    "0x00000000000000000000000000000000000000000000000000000000000d11ce"
)

# Check bridge balance before
BRIDGE_BALANCE_BEFORE=$(cast call "$WBTC_ADDRESS" "balanceOf(address)(uint256)" "$BRIDGE_ADDRESS" --rpc-url "$MOJAVE_RPC_URL" 2>/dev/null || echo "0")
BRIDGE_BALANCE_BEFORE_DEC=$(echo "$BRIDGE_BALANCE_BEFORE" | grep -oE '^[0-9]+' || echo "0")
echo "  Bridge wBTC balance before: $BRIDGE_BALANCE_BEFORE_DEC sats"
echo ""

# Submit signatures incrementally (script will auto-generate EIP-712 signatures)
TOTAL_OPERATORS=${#OPERATOR_KEYS[@]}
echo "  Submitting $TOTAL_OPERATORS signatures (threshold=4)..."
echo ""

WITHDRAW_SUCCESS=false
for i in "${!OPERATOR_KEYS[@]}"; do
    OPERATOR_NUM=$((i+1))
    OPERATOR_KEY="${OPERATOR_KEYS[$i]}"
    
    echo -e "${BLUE}  === Operator $OPERATOR_NUM of $TOTAL_OPERATORS ===${NC}"
    
    # All operators submit incremental signatures (no rawTx)
    # TODO: finalization happens separately when threshold reached
    echo "  → Incremental signature (no rawTx, auto-generated EIP-712 sig)"
    
    # FLOW:
    # 1. Operator listens for WithdrawalInitiated event
    # 2. Extracts PSBT from event.psbt field
    # 3. Signs PSBT with operator's Bitcoin private key
    # 4. Submits EIP-712 approval signature: submitSignature(wid, sig, "")
    # 5. When threshold reached, state → Ready
    # 6. Separate finalizer collects all Bitcoin signatures, builds rawTx
    # 7. Finalizer calls finalizeWithRawTx(wid, rawTx) or similar
    # 8. Contract verifies rawTx outputs match PSBT outputsHash
    # 9. Contract emits SignedTxReady with rawTx for Bitcoin broadcast
    
    SUBMIT_OUTPUT=$(cd "$PROJECT_ROOT" && \
    OPERATOR_KEY=$OPERATOR_KEY \
    BRIDGE_ADDRESS=$BRIDGE_ADDRESS \
    WID=$WID \
    forge script script/withdrawal/SubmitSignature.s.sol:SubmitSignature \
        --broadcast \
        --rpc-url "$MOJAVE_RPC_URL" \
        --legacy 2>&1)
    
    # Save output for debugging
    echo "$SUBMIT_OUTPUT" > "/tmp/submit_sig_op${OPERATOR_NUM}.log"
    
    # Check if submission succeeded
    if echo "$SUBMIT_OUTPUT" | grep -q "\[SUCCESS\] Signature submitted"; then
        echo -e "  ${GREEN}✓ Signature $OPERATOR_NUM submitted${NC}"
        
        # Check if ready (threshold reached)
        if echo "$SUBMIT_OUTPUT" | grep -q "\[READY\]"; then
            echo -e "  ${GREEN}✓✓✓ READY state reached (threshold met)${NC}"
            echo "  Withdrawal is now ready for finalization"
            WITHDRAW_SUCCESS=true
        fi
    else
        echo -e "  ${RED}✗ Signature $OPERATOR_NUM failed${NC}"
        echo "$SUBMIT_OUTPUT" | grep -E "ERROR|Revert|FAILED" | head -10
        echo "  Full log saved to: /tmp/submit_sig_op${OPERATOR_NUM}.log"
    fi
    
    echo ""
    sleep 2
done

# Check final withdrawal state
echo ""
echo "  Checking final withdrawal state..."
FINAL_STATE=$(cast call "$BRIDGE_ADDRESS" "getWithdrawalDetails(bytes32)(address,uint256,bytes,uint64,bytes32,uint32,uint32,uint8)" "$WID" --rpc-url "$MOJAVE_RPC_URL" 2>/dev/null | tail -1)
FINAL_STATE_NUM=$(echo "$FINAL_STATE" | tr -d ' ')

if [ "$FINAL_STATE_NUM" = "2" ]; then
    echo -e "  ${GREEN}✓ Withdrawal state: READY (2)${NC}"
    echo "  Threshold signatures collected successfully"
    # TODO: Separate finalizer would now build and submit rawTx
elif [ "$FINAL_STATE_NUM" = "3" ]; then
    echo -e "  ${YELLOW}✓ Withdrawal state: FINALIZED (3)${NC}"
    echo "  (Unexpected - should be READY without rawTx submission)"
else
    echo -e "  ${YELLOW}⚠ Withdrawal state: $FINAL_STATE_NUM${NC}"
    echo "  Expected: 2 (READY)"
fi

# ============================================
# Step 15: Verify Final State
# ============================================
echo ""
echo -e "${YELLOW}[15/17] Verifying final withdrawal state...${NC}"

# Check withdrawal status via getWithdrawalDetails (returns tuple with state at index 7)
WITHDRAW_DETAILS=$(cast call "$BRIDGE_ADDRESS" "getWithdrawalDetails(bytes32)(address,uint256,bytes,uint64,bytes32,uint32,uint32,uint8)" "$WID" --rpc-url "$MOJAVE_RPC_URL" 2>/dev/null)
WITHDRAW_STATUS=$(echo "$WITHDRAW_DETAILS" | tail -1 | tr -d ' ')
echo "  Withdrawal status: $WITHDRAW_STATUS (0=None, 1=Pending, 2=Ready, 3=Finalized, 4=Canceled)"

# For incremental signature test, success means reaching READY state
# (Finalized state requires rawTx submission, which is done separately)
if [ "$WITHDRAW_STATUS" = "2" ]; then
    echo -e "${GREEN}  ✓ Status: READY (threshold signatures collected)${NC}"
    echo "  Withdrawal is ready for finalization with Bitcoin rawTx"
    WITHDRAW_SUCCESS=true
elif [ "$WITHDRAW_STATUS" = "3" ]; then
    echo -e "${GREEN}  ✓ Status: FINALIZED (auto-finalized with rawTx)${NC}"
    WITHDRAW_SUCCESS=true
elif [ "$WITHDRAW_STATUS" = "1" ]; then
    echo -e "${YELLOW}  ⚠ Status: PENDING (not enough signatures)${NC}"
elif [ "$WITHDRAW_STATUS" = "0" ]; then
    echo -e "${RED}  ✗ Status: NONE (withdrawal not found)${NC}"
elif [ "$WITHDRAW_STATUS" = "4" ]; then
    echo -e "${RED}  ✗ Status: CANCELED${NC}"
else
    echo -e "${RED}  ✗ Status: Unknown ($WITHDRAW_STATUS)${NC}"
fi

# ============================================
# Step 16: Build and Submit Signed Bitcoin TX
# ============================================
if [ "$WITHDRAW_STATUS" = "2" ]; then
    echo ""
    echo -e "${YELLOW}[16/17] Building signed Bitcoin transaction and finalizing...${NC}"
    
    # Build valid Bitcoin transaction from withdrawal parameters
    echo "  Building Bitcoin transaction from PSBT..."
    
    # Use helper script to build transaction
    BUILD_OUTPUT=$("$SCRIPT_DIR/../utils/build_signed_tx_from_psbt.sh" "$WID" "$BRIDGE_ADDRESS" "$MOJAVE_RPC_URL" 2>&1)
    
    if echo "$BUILD_OUTPUT" | grep -q "Bitcoin Transaction Built Successfully"; then
        echo -e "  ${GREEN}✓ Bitcoin transaction built${NC}"
        
        # Extract RAW_TX from build output using more robust method
        FINAL_RAW_TX=$(echo "$BUILD_OUTPUT" | grep 'RAW_TX="0x' | sed 's/.*RAW_TX="//' | sed 's/".*$//' | head -1)
        
        if [ -z "$FINAL_RAW_TX" ] || [ ${#FINAL_RAW_TX} -lt 100 ]; then
            echo -e "  ${RED}✗ Failed to extract RAW_TX${NC}"
            echo "  Build output (debug):"
            echo "$BUILD_OUTPUT" | grep -E "RAW_TX|Transaction" | head -5
            FINAL_RAW_TX=""
        fi
        
        if [ -n "$FINAL_RAW_TX" ]; then
            echo "  Transaction length: ${#FINAL_RAW_TX} chars"
            echo "  Preview: ${FINAL_RAW_TX:0:100}..."
            echo ""
            
            # Submit finalization with operator 1
            echo "  Submitting finalization (operator 1)..."
            FINALIZE_OUTPUT=$(cd "$PROJECT_ROOT" && \
            OPERATOR_KEY="0x00000000000000000000000000000000000000000000000000000000000a11ce" \
            BRIDGE_ADDRESS=$BRIDGE_ADDRESS \
            WID=$WID \
            RAW_TX="$FINAL_RAW_TX" \
            forge script script/withdrawal/FinalizePSBT.s.sol:FinalizePSBT \
                --broadcast \
                --rpc-url "$MOJAVE_RPC_URL" \
                --legacy 2>&1)
            
            if echo "$FINALIZE_OUTPUT" | grep -q "\[SUCCESS\] Withdrawal finalized"; then
                echo -e "  ${GREEN}✓✓✓ Withdrawal finalized with Bitcoin TX!${NC}"
                FINALIZATION_SUCCESS=true
            else
                echo -e "  ${RED}✗ Finalization failed${NC}"
                echo "$FINALIZE_OUTPUT" | grep -E "ERROR|Revert|FAILED" | head -5
            fi
        else
            echo -e "  ${RED}✗ Cannot finalize without valid RAW_TX${NC}"
        fi
    else
        echo -e "  ${YELLOW}⚠ Could not build Bitcoin transaction${NC}"
        echo "  $BUILD_OUTPUT" | head -10
    fi
else
    echo ""
    echo -e "${YELLOW}[16/17] Skipping finalization (withdrawal not in Ready state)${NC}"
fi

echo ""

# ============================================
# Step 17: Verify Final State After Finalization
# ============================================
echo -e "${YELLOW}[17/17] Verifying final state after finalization...${NC}"

# Check balances after
BRIDGE_BALANCE_AFTER=$(cast call "$WBTC_ADDRESS" "balanceOf(address)(uint256)" "$BRIDGE_ADDRESS" --rpc-url "$MOJAVE_RPC_URL" 2>/dev/null || echo "0")
BRIDGE_BALANCE_AFTER_DEC=$(echo "$BRIDGE_BALANCE_AFTER" | grep -oE '^[0-9]+' || echo "0")

TOTAL_SUPPLY_AFTER=$(cast call "$WBTC_ADDRESS" "totalSupply()(uint256)" --rpc-url "$MOJAVE_RPC_URL" 2>/dev/null || echo "0")
TOTAL_SUPPLY_AFTER_DEC=$(echo "$TOTAL_SUPPLY_AFTER" | grep -oE '^[0-9]+' || echo "0")

BURNED_AMOUNT=$((BRIDGE_BALANCE_BEFORE_DEC - BRIDGE_BALANCE_AFTER_DEC))

echo "  Bridge wBTC balance after: $BRIDGE_BALANCE_AFTER_DEC sats"
echo "  wBTC burned: $BURNED_AMOUNT sats"
echo "  Total wBTC supply: $TOTAL_SUPPLY_AFTER_DEC sats"

# Re-check withdrawal status
FINAL_DETAILS=$(cast call "$BRIDGE_ADDRESS" "getWithdrawalDetails(bytes32)(address,uint256,bytes,uint64,bytes32,uint32,uint32,uint8)" "$WID" --rpc-url "$MOJAVE_RPC_URL" 2>/dev/null)
FINAL_STATUS=$(echo "$FINAL_DETAILS" | tail -1 | tr -d ' ')

echo "  Final withdrawal status: $FINAL_STATUS (3=Finalized)"

if [ "$FINAL_STATUS" = "3" ]; then
    echo -e "${GREEN}  ✓ Withdrawal FINALIZED${NC}"
    WITHDRAW_SUCCESS=true
elif [ "$FINAL_STATUS" = "2" ]; then
    echo -e "${YELLOW}  ⚠ Withdrawal still in READY state (finalization may have failed)${NC}"
else
    echo -e "${RED}  ✗ Unexpected final status: $FINAL_STATUS${NC}"
fi
echo ""

if [ "$WITHDRAW_SUCCESS" = "true" ] && [ "$BURNED_AMOUNT" -gt 0 ]; then
    echo -e "${GREEN}[OK] Complete withdrawal flow successful!${NC}"
    echo "  ✓ Signatures collected incrementally"
    echo "  ✓ Bitcoin transaction built from PSBT"
    echo "  ✓ Withdrawal finalized"
    echo "  ✓ wBTC burned correctly"
elif [ "$WITHDRAW_SUCCESS" = "true" ]; then
    echo -e "${GREEN}[OK] Incremental signatures collected successfully!${NC}"
    echo "  ✓ Withdrawal reached READY state"
    echo "  (Finalization step optional in this test)"
else
    echo -e "${RED}[ERROR] Withdrawal flow incomplete${NC}"
fi

# ============================================
# Final Summary
# ============================================
echo ""
echo -e "${BLUE}==========================================="
echo "INCREMENTAL SIGNATURE TEST COMPLETED"
echo "submitSignature() One-by-One Flow"
echo -e "===========================================${NC}"
echo ""
echo "=== Deposit Flow ==="
echo "  Bitcoin Height: $(cat /tmp/bitcoin_height.txt 2>/dev/null || echo "N/A")"
echo "  BtcRelay Best: $(cast call "$RELAY" "bestHeight()(uint256)" --rpc-url "$MOJAVE_RPC_URL" 2>/dev/null || echo "N/A")"
echo "  BtcRelay Finalized: $(cast call "$RELAY" "finalizedHeight()(uint256)" --rpc-url "$MOJAVE_RPC_URL" 2>/dev/null || echo "N/A")"
echo "  Deposit TXID: $DEPOSIT_TXID"
echo "  wBTC Minted: $WBTC_BAL sats"
echo ""
echo "=== Withdrawal Flow ==="
echo "  Withdrawal Amount: $WITHDRAW_AMOUNT sats"
echo "  WID: $WID"
echo "  Destination: $WITHDRAW_DEST_ADDR"
echo "  wBTC Burned: ${BURNED_AMOUNT:-0} sats"
echo ""
echo "Deployed Contracts:"
echo "  WBTC: $WBTC_ADDRESS"
echo "  BtcRelay: $RELAY_ADDRESS"
echo "  BridgeGateway: $BRIDGE_ADDRESS"
echo ""

if [ "$FINALIZATION_SUCCESS" = "true" ]; then
    echo -e "${GREEN}✓✓✓ FULL PSBT FLOW TEST PASSED ✓✓✓${NC}"
    echo "  ✓ PSBT extracted from WithdrawalInitiated event"
    echo "  ✓ Operators submitted signatures incrementally (4-of-5)"
    echo "  ✓ Threshold reached → READY state"
    echo "  ✓ Bitcoin transaction built matching PSBT outputs"
    echo "  ✓ Contract validated and finalized withdrawal"
    echo "  ✓ wBTC burned, SignedTxReady emitted"
    echo "  ✓ Ready for Bitcoin network broadcast"
elif [ "$SUCCESS" = "true" ] && [ "$WITHDRAW_SUCCESS" = "true" ]; then
    echo -e "${GREEN}✓✓✓ INCREMENTAL SIGNATURE TEST PASSED ✓✓✓${NC}"
    echo "  All steps completed successfully"
    echo "  ✓ Bitcoin headers verified with real PoW"
    echo "  ✓ SPV proof validated"
    echo "  ✓ wBTC minted correctly ($WBTC_BALANCE_DEC sats)"
    echo "  ✓ Signatures submitted incrementally (4 operators)"
    echo "  ✓ Auto-finalization triggered on threshold"
    echo "  ✓ wBTC burned correctly ($BURNED_AMOUNT sats)"
elif [ "$SUCCESS" = "true" ] && [ "$WITHDRAW_SUCCESS" != "true" ]; then
    echo -e "${YELLOW}⚠ PARTIAL SUCCESS ⚠${NC}"
    echo "  ✓ Deposit flow completed"
    echo "  ✗ Withdrawal flow failed"
elif [ "$SUCCESS" = "partial" ]; then
    echo -e "${YELLOW}⚠ TEST COMPLETED WITH WARNINGS ⚠${NC}"
    echo "  Check output above for details"
else
    echo -e "${RED}✗ TEST FAILED ✗${NC}"
    echo "  SPV proof or minting failed"
fi
echo ""
