#!/bin/bash
set -e  # Exit on error

# ================================
# Run e2e tests
# ================================

echo "Running e2e tests"

echo "Checking if rex is installed"
if ! command -v rex >/dev/null 2>&1; then
  REX_REPOSITORY_URL="https://github.com/lambdaclass/rex"
  REX_CLI_DOC_URL="${REX_REPOSITORY_URL}/blob/main/cli/README.md"

cat >&2 <<EOF
ERROR: 'rex' command not found. ❌

See the installation guides:
  - Repository: ${REX_REPOSITORY_URL}
  - CLI README: ${REX_CLI_DOC_URL}

After installation, ensure 'rex' is on your PATH (e.g., \`rex --version\`) and re-run the script.
EOF
  exit 127
fi

# Source code: https://github.com/Drizzle210/Counter/blob/main/src/Counter.sol
COUNTER_CONTRACT_BYTE_CODE=0x6080604052348015600e575f5ffd5b5060015f819055506101e1806100235f395ff3fe608060405234801561000f575f5ffd5b506004361061003f575f3560e01c80633fb5c1cb146100435780638381f58a1461005f578063d09de08a1461007d575b5f5ffd5b61005d600480360381019061005891906100e4565b610087565b005b610067610090565b604051610074919061011e565b60405180910390f35b610085610095565b005b805f8190555050565b5f5481565b5f5f8154809291906100a690610164565b9190505550565b5f5ffd5b5f819050919050565b6100c3816100b1565b81146100cd575f5ffd5b50565b5f813590506100de816100ba565b92915050565b5f602082840312156100f9576100f86100ad565b5b5f610106848285016100d0565b91505092915050565b610118816100b1565b82525050565b5f6020820190506101315f83018461010f565b92915050565b7f4e487b71000000000000000000000000000000000000000000000000000000005f52601160045260245ffd5b5f61016e826100b1565b91507fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff82036101a05761019f610137565b5b60018201905091905056fea2646970667358221220eae701fa18f33b036fc7f42e24ebce60615e3e6f6d8299b230408e1f64eed2bf64736f6c634300081e0033
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

# discv4/RLPx readiness: detect when the sequencer's P2P stack is up
# Actively probes the TCP RLPx port; optionally infers port from logs.
can_connect_tcp() {
    local host="$1" port="$2"
    if command -v nc >/dev/null 2>&1; then
        nc -z "$host" "$port" >/dev/null 2>&1
        return $?
    fi
    # Fallback to bash's /dev/tcp
    (exec 3<>"/dev/tcp/${host}/${port}") >/dev/null 2>&1
}

wait_for_discv4() {
    local host
    host=$(ip addr show | grep "inet " | grep -v 127.0.0.1 | awk '{print $2}' | cut -d/ -f1 | head -n1)
    local p2p_port="${2:-}"
    local log_file="${3:-.mojave/sequencer.log}"
    local timeout="${4:-120}"
    local elapsed=0

    while (( elapsed < timeout )); do
        # Infer port from enode in logs if not provided yet
        if [ -z "$p2p_port" ] && [ -f "$log_file" ]; then
            p2p_port=$(grep -Eo 'enode://[^ ]+@[0-9.]+:[0-9]+' "$log_file" 2>/dev/null | tail -n1 | sed -E 's/.*:([0-9]+)$/\1/')
        fi
        # Fallback to default P2P port used by justfile if still empty
        if [ -z "$p2p_port" ]; then p2p_port=30305; fi

        if can_connect_tcp "$host" "$p2p_port"; then
            return 0
        fi
        sleep 2
        elapsed=$((elapsed + 2))
    done
    return 1
}

echo "Waiting for sequencer readiness (discv4/RLPx)..."
if ! wait_for_discv4 "127.0.0.1" "30305" ".mojave/sequencer.log" 120; then
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
RPC_URL="http://localhost:8545"
CONTRACT_ADDRESS=$(rex deploy --bytecode "$COUNTER_CONTRACT_BYTE_CODE" 0 "$PRIVATE_KEY" --rpc-url "$RPC_URL" --print-address)

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
