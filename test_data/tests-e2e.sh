#!/bin/bash
set -e  # Exit on error

# Kill any existing mojave processes
cleanup() {
    pkill -f mojave || true
    sleep 2
}

cleanup

echo "Starting all services"

# Start sequencer on port 1739
ETHREX_HTTP_PORT=1739 just sequencer &
sleep 2 

# Start node on port 8545 with correct sequencer connection
ETHREX_HTTP_PORT=8545 just node &
sleep 2

# # Start prover
# just prover &
# sleep 2



echo "All services started. Testing connection..."


# ================================
# Start testing
# ================================


# Test connection to sequencer
response=$(curl -s -X POST -H "Content-Type: application/json" \
    --data '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}' \
    http://localhost:1739)

echo "Response from sequencer: $response"

echo "Testing send raw transaction to full node"

# Create a signed EIP1559 transaction to send to full node
# The full node will automatically forward it to the sequencer
# The params is extracted from the test_forward_transaction() in mod.rs
tx_data=$(cat << 'EOF'
{
    "jsonrpc": "2.0",
    "method": "eth_sendRawTransaction",
    "params": ["0x02f8758206c18084773594008506fc23ac00825208940000000000000000000000000000000000000001880de0b6b3a764000080c001a0c2b251be76bed6798f93a7441d4bc5fc5c9801ba7f16398ed174a99eb0f01ea7a02d053e77b6e557ba04d009109c42cd94fa2d8f65d465c727254a18276c6812dc"],
    "id": 1
}
EOF
)

# Send to full node on port 8545, which will forward to sequencer on port 1739
echo "Sending transaction to full node which will forward to sequencer..."
response=$(curl -s -X POST -H "Content-Type: application/json" \
    --data "$tx_data" \
    http://localhost:8545)

echo "Response from transaction send: $response"

# Verify the transaction hash matches expected
expected_hash="0x81c611445d4de5c61f74bc286f5b04d8334b60e1d7e0b29ad6b9c524e1ae430b"
actual_hash=$(echo "$response" | jq -r '.result')

if [ "$actual_hash" = "$expected_hash" ]; then
    echo "Transaction hash matches expected value"
else
    echo "ERROR: Transaction hash does not match expected value"
    echo "Expected: $expected_hash"
    echo "Got: $actual_hash"

    cleanup

    exit 1
fi

# ================================
# Clean up
# ================================

cleanup

echo "TEST PASSED."