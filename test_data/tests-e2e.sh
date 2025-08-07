#!/bin/bash
set -e  # Exit on error

# ================================
# Run unit tests
# ================================

echo "Running unit tests"

cargo test --workspace || (echo "Unit tests failed"; exit 1)

# ================================
# Run e2e tests
# ================================

echo "Running e2e tests"


COUNTER_CONTRACT_BYTE_CODE=0x6080604052348015600e575f5ffd5b5060015f819055506101e1806100235f395ff3fe608060405234801561000f575f5ffd5b506004361061003f575f3560e01c80633fb5c1cb146100435780638381f58a1461005f578063d09de08a1461007d575b5f5ffd5b61005d600480360381019061005891906100e4565b610087565b005b610067610090565b604051610074919061011e565b60405180910390f35b610085610095565b005b805f8190555050565b5f5481565b5f5f8154809291906100a690610164565b9190505550565b5f5ffd5b5f819050919050565b6100c3816100b1565b81146100cd575f5ffd5b50565b5f813590506100de816100ba565b92915050565b5f602082840312156100f9576100f86100ad565b5b5f610106848285016100d0565b91505092915050565b610118816100b1565b82525050565b5f6020820190506101315f83018461010f565b92915050565b7f4e487b71000000000000000000000000000000000000000000000000000000005f52601160045260245ffd5b5f61016e826100b1565b91507fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff82036101a05761019f610137565b5b60018201905091905056fea264697066735822122035036984d9cda9d8b7e24e9a8d12e922bc40578a5e3f11d8594eefda13b952f864736f6c634300081c0033
PRIVATE_KEY=0xc97833ebdbc5d3b280eaee0c826f2bd3b5959fb902d60a167d75a035c694f282

# ================================
# Start services
# ================================

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




# Deploy with bytecode
CONTRACT_ADDRESS=$(rex deploy "$BYTE_CODE" 0 "$PRIVATE_KEY" --print-address)

echo "Contract address: $CONTRACT_ADDRESS"

# Get number and check it equals 1
NUMBER=$(rex call "$CONTRACT_ADDRESS" --calldata 0x8381f58a)
if [ "$NUMBER" = "0x0000000000000000000000000000000000000000000000000000000000000001" ]; then
    echo "Initial number is 1"
else
    echo "ERROR: Initial number is not 1"
    echo "Expected: 0x0000000000000000000000000000000000000000000000000000000000000001"
    echo "Got: $NUMBER"

    cleanup
    exit 1
fi

# Set number to 5
rex send "$CONTRACT_ADDRESS" 0 "$PRIVATE_KEY" --calldata 0x3fb5c1cb0000000000000000000000000000000000000000000000000000000000000005 

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

    cleanup
    exit 1
fi


# ================================
# Clean up
# ================================

cleanup

echo "ALL TESTS PASSED SUCCESSFULLY!"