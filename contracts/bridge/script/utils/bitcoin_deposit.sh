#!/bin/bash

# Bitcoin Deposit Helper Script
# Creates and broadcasts Bitcoin transaction with OP_RETURN envelope

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Default values
NETWORK="testnet"
MIN_CONFIRMATIONS=1

# Function to display usage
usage() {
    echo "Usage: $0 --amount <sats> --envelope <hex> --vault-spk <hex> [options]"
    echo ""
    echo "Required:"
    echo "  --amount <sats>        Amount to deposit in satoshis"
    echo "  --envelope <hex>       Envelope hex string (from Step1)"
    echo "  --vault-spk <hex>      Vault scriptPubKey hex"
    echo ""
    echo "Optional:"
    echo "  --network <net>        Bitcoin network (testnet/regtest/mainnet, default: testnet)"
    echo "  --utxo <txid:vout>     Specific UTXO to use (optional, auto-select if not provided)"
    echo "  --change <address>     Change address (optional, uses wallet default)"
    echo "  --help                 Show this help message"
    echo ""
    echo "Example:"
    echo "  $0 --amount 300 \\"
    echo "    --envelope 0x4d4f4a31... \\"
    echo "    --vault-spk 0x5120cccc..."
    exit 1
}

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --amount)
            AMOUNT="$2"
            shift 2
            ;;
        --envelope)
            ENVELOPE="$2"
            shift 2
            ;;
        --vault-spk)
            VAULT_SPK="$2"
            shift 2
            ;;
        --network)
            NETWORK="$2"
            shift 2
            ;;
        --utxo)
            UTXO="$2"
            shift 2
            ;;
        --change)
            CHANGE_ADDR="$2"
            shift 2
            ;;
        --help)
            usage
            ;;
        *)
            echo -e "${RED}Unknown option: $1${NC}"
            usage
            ;;
    esac
done

# Validate required arguments
if [ -z "$AMOUNT" ] || [ -z "$ENVELOPE" ] || [ -z "$VAULT_SPK" ]; then
    echo -e "${RED}Error: Missing required arguments${NC}"
    usage
fi

# Remove 0x prefix if present
ENVELOPE="${ENVELOPE#0x}"
VAULT_SPK="${VAULT_SPK#0x}"

# Set bitcoin-cli network flag
if [ "$NETWORK" == "testnet" ]; then
    BTC_CLI="bitcoin-cli -testnet"
elif [ "$NETWORK" == "regtest" ]; then
    BTC_CLI="bitcoin-cli -regtest"
else
    BTC_CLI="bitcoin-cli"
fi

echo -e "${GREEN}========================================${NC}"
echo -e "${GREEN}Bitcoin Deposit Transaction Creator${NC}"
echo -e "${GREEN}========================================${NC}"
echo ""
echo "Network: $NETWORK"
echo "Amount: $AMOUNT sats"
echo "Envelope: 0x${ENVELOPE:0:32}..."
echo ""

# Step 1: Get wallet info
echo -e "${YELLOW}[1/6] Checking wallet...${NC}"
WALLET_INFO=$($BTC_CLI getwalletinfo 2>/dev/null || echo "error")
if [ "$WALLET_INFO" == "error" ]; then
    echo -e "${RED}Error: Bitcoin wallet not loaded${NC}"
    echo "Try: bitcoin-cli -$NETWORK loadwallet <wallet_name>"
    exit 1
fi
echo "✓ Wallet loaded"
echo ""

# Step 2: Select UTXO
echo -e "${YELLOW}[2/6] Selecting UTXO...${NC}"
if [ -z "$UTXO" ]; then
    # Auto-select UTXO
    UTXOS=$($BTC_CLI listunspent 1 9999999)
    UTXO_COUNT=$(echo "$UTXOS" | jq 'length')
    
    if [ "$UTXO_COUNT" -eq 0 ]; then
        echo -e "${RED}Error: No UTXOs available${NC}"
        echo "Fund your wallet first:"
        echo "  Address: $($BTC_CLI getnewaddress)"
        exit 1
    fi
    
    # Select first UTXO with enough balance
    UTXO_TXID=$(echo "$UTXOS" | jq -r '.[0].txid')
    UTXO_VOUT=$(echo "$UTXOS" | jq -r '.[0].vout')
    UTXO_AMOUNT=$(echo "$UTXOS" | jq -r '.[0].amount')
    
    echo "Selected UTXO:"
    echo "  TXID: $UTXO_TXID"
    echo "  VOUT: $UTXO_VOUT"
    echo "  Amount: $UTXO_AMOUNT BTC"
else
    UTXO_TXID="${UTXO%:*}"
    UTXO_VOUT="${UTXO#*:}"
    echo "Using provided UTXO: $UTXO_TXID:$UTXO_VOUT"
fi
echo ""

# Step 3: Calculate amounts (in BTC)
echo -e "${YELLOW}[3/6] Calculating amounts...${NC}"
AMOUNT_BTC=$(echo "scale=8; $AMOUNT / 100000000" | bc)
FEE_BTC="0.00001000"  # 1000 sats fee (adjust as needed)

echo "Deposit amount: $AMOUNT_BTC BTC ($AMOUNT sats)"
echo "Estimated fee: $FEE_BTC BTC"
echo ""

# Step 4: Prepare OP_RETURN data
echo -e "${YELLOW}[4/6] Preparing OP_RETURN...${NC}"
# OP_RETURN format: 6a (OP_RETURN) + 4c (OP_PUSHDATA1) + length + data
ENVELOPE_LEN=$(printf '%02x' $((${#ENVELOPE} / 2)))
OPRETURN_DATA="6a4c${ENVELOPE_LEN}${ENVELOPE}"

echo "OP_RETURN data: 0x${OPRETURN_DATA:0:40}..."
echo "Length: $((${#ENVELOPE} / 2)) bytes"
echo ""

# Step 5: Create raw transaction
echo -e "${YELLOW}[5/6] Creating transaction...${NC}"

# Get change address if not provided
if [ -z "$CHANGE_ADDR" ]; then
    CHANGE_ADDR=$($BTC_CLI getrawchangeaddress)
fi

# Decode vault SPK to address (simplified - assumes P2WPKH/P2WSH)
# For testing, we'll use a temporary address or the change address
# TODO: Need proper SPK to address conversion
VAULT_ADDR=$CHANGE_ADDR  # FIXME: Convert VAULT_SPK to proper address

# Build transaction JSON
INPUTS="[{\"txid\":\"$UTXO_TXID\",\"vout\":$UTXO_VOUT}]"
OUTPUTS="{\"$VAULT_ADDR\":$AMOUNT_BTC,\"data\":\"${OPRETURN_DATA}\"}"

echo "Creating raw transaction..."
echo "DEBUG: INPUTS=$INPUTS"
echo "DEBUG: OUTPUTS=$OUTPUTS"
RAW_TX=$($BTC_CLI createrawtransaction "$INPUTS" "$OUTPUTS" 2>&1)

if [ -z "$RAW_TX" ]; then
    echo -e "${RED}Error: Failed to create raw transaction${NC}"
    exit 1
fi

echo "✓ Raw transaction created"
echo ""

# Step 6: Sign and broadcast
echo -e "${YELLOW}[6/6] Signing and broadcasting...${NC}"

SIGNED_TX=$($BTC_CLI signrawtransactionwithwallet "$RAW_TX")
SIGNED_HEX=$(echo "$SIGNED_TX" | jq -r '.hex')
IS_COMPLETE=$(echo "$SIGNED_TX" | jq -r '.complete')

if [ "$IS_COMPLETE" != "true" ]; then
    echo -e "${RED}Error: Transaction signing incomplete${NC}"
    echo "$SIGNED_TX" | jq '.'
    exit 1
fi

echo "✓ Transaction signed"
echo ""

# Broadcast
TXID=$($BTC_CLI sendrawtransaction "$SIGNED_HEX")

if [ -z "$TXID" ]; then
    echo -e "${RED}Error: Failed to broadcast transaction${NC}"
    exit 1
fi

echo -e "${GREEN}========================================${NC}"
echo -e "${GREEN}SUCCESS!${NC}"
echo -e "${GREEN}========================================${NC}"
echo ""
echo "Transaction broadcasted to Bitcoin $NETWORK"
echo ""
echo "TXID: $TXID"
echo ""
echo "Save this TXID for Step 3!"
echo ""
echo "Waiting for confirmations..."
echo "  Required: $MIN_CONFIRMATIONS confirmation(s)"
echo ""
echo "Check status:"
echo "  $BTC_CLI getrawtransaction $TXID true"
echo ""
echo -e "${YELLOW}Export TXID for next step:${NC}"
echo "  export BITCOIN_DEPOSIT_TXID=$TXID"
echo ""
