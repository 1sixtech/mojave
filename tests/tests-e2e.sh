#!/bin/bash
set -e  # Exit on error

# ================================
# Run e2e tests
# ================================

echo "Running e2e tests"

echo "Checking if rex is installed"
REX_PATH=$(command -v rex 2>/dev/null || true)
if [ -z "$REX_PATH" ]; then
    echo "ERROR: 'rex' command not found!"
    exit 1
fi

# Source code: https://github.com/Drizzle210/Counter/blob/main/src/Counter.sol
COUNTER_CONTRACT_BYTE_CODE=0x6080604052348015600e575f5ffd5b5060015f819055506101e1806100235f395ff3fe608060405234801561000f575f5ffd5b506004361061003f575f3560e01c80633fb5c1cb146100435780638381f58a1461005f578063d09de08a1461007d575b5f5ffd5b61005d600480360381019061005891906100e4565b610087565b005b610067610090565b604051610074919061011e565b60405180910390f35b610085610095565b005b805f8190555050565b5f5481565b5f5f8154809291906100a690610164565b9190505550565b5f5ffd5b5f819050919050565b6100c3816100b1565b81146100cd575f5ffd5b50565b5f813590506100de816100ba565b92915050565b5f602082840312156100f9576100f86100ad565b5b5f610106848285016100d0565b91505092915050565b610118816100b1565b82525050565b5f6020820190506101315f83018461010f565b92915050565b7f4e487b71000000000000000000000000000000000000000000000000000000005f52601160045260245ffd5b5f61016e826100b1565b91507fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff82036101a05761019f610137565b5b60018201905091905056fea264697066735822122035036984d9cda9d8b7e24e9a8d12e922bc40578a5e3f11d8594eefda13b952f864736f6c634300081c0033
PRIVATE_KEY=0xc97833ebdbc5d3b280eaee0c826f2bd3b5959fb902d60a167d75a035c694f282

# ================================
# Start services
# ================================

# Kill any existing mojave processes
cleanup() {
    pkill -f mojave-full-node || true
    pkill -f mojave-sequencer || true
    sleep 2
}

echo "Starting all services"

bash scripts/start.sh &

# Wait for services to be ready
wait_for_jsonrpc() {
    local url="$1"
    local timeout="${2:-120}"
    local elapsed=0
    while (( elapsed < timeout )); do
        if curl -fsS --max-time 2 -H "Content-Type: application/json" \
            -d '{"jsonrpc":"2.0","id":1,"method":"web3_clientVersion","params":[]}' \
            "$url" >/dev/null 2>&1; then
            return 0
        fi
        sleep 2
        elapsed=$((elapsed + 2))
    done
    return 1
}

echo "Waiting for sequencer readiness..."
if ! wait_for_jsonrpc "http://localhost:1739" 120; then
    echo "ERROR: Sequencer did not become ready in time"
    exit 1
fi

echo "Waiting for full node readiness..."
if ! wait_for_jsonrpc "http://localhost:8545" 120; then
    echo "ERROR: Full node did not become ready in time"
    exit 1
fi

echo "Both services are ready."

echo "All services started. Testing connection..."

trap cleanup INT TERM EXIT


# ================================
# Start testing
# ================================

# Deploy with bytecode
CONTRACT_ADDRESS=$(rex deploy "$COUNTER_CONTRACT_BYTE_CODE" 0 "$PRIVATE_KEY" --print-address)

echo "Contract address: $CONTRACT_ADDRESS"

# Get number and check it equals 1
NUMBER=$(rex call "$CONTRACT_ADDRESS" --calldata 0x8381f58a)
if [ "$NUMBER" = "0x0000000000000000000000000000000000000000000000000000000000000001" ]; then
    echo "Initial number is 1"
else
    echo "ERROR: Initial number is not 1"
    echo "Expected: 0x0000000000000000000000000000000000000000000000000000000000000001"
    echo "Got: $NUMBER"

    exit 1
fi

# Set number to 5
rex send "$CONTRACT_ADDRESS" 0 --calldata 0x3fb5c1cb0000000000000000000000000000000000000000000000000000000000000005 --private-key "$PRIVATE_KEY"

# sleep to wait for the transaction to be mined
sleep 2

# Get number and check it equals 5
NUMBER=$(rex call "$CONTRACT_ADDRESS" --calldata 0x8381f58a)
if [ "$NUMBER" = "0x0000000000000000000000000000000000000000000000000000000000000005" ]; then
    echo "Number is 5"
else
    echo "ERROR: Number is not 5"
    echo "Expected: 0x0000000000000000000000000000000000000000000000000000000000000005"
    echo "Got: $NUMBER"

    exit 1
fi


# ================================
# Clean up
# ================================

echo "ALL TESTS PASSED SUCCESSFULLY!"   