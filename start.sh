NODE_HOST="127.0.0.1"
SEQ_HOST="127.0.0.1"
NODE_PORT="8545"
NODE_P2P_PORT="30304"
SEQ_PORT="1739"

NODE_HTTP="http://${NODE_HOST}:${NODE_PORT}"
SEQ_HTTP="http://${SEQ_HOST}:${SEQ_PORT}"
GENESIS="./data/testnet-genesis.json"
NODE_DATA_DIR="$(pwd)/mojave-node"
SEQ_DATA_DIR="$(pwd)/mojave-sequencer"
SEQ_PRIVKEY="${SEQ_PRIVKEY:-0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa}"
NODE_READY_TIMEOUT=120
SEQ_READY_TIMEOUT=60
CHECK_INTERVAL=2
RED='\033[0;31m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
NC='\033[0m'
NODE_PIPE="$(mktemp -u)"
SEQUENCER_PIPE="$(mktemp -u)"
mkfifo "$NODE_PIPE" "$SEQUENCER_PIPE"
NODE_LOG="$(mktemp -t node.log.XXXXXX)"
SEQ_LOG="$(mktemp -t sequencer.log.XXXXXX)"
NODE_PID=""
SEQUENCER_PID=""
LOG_NODE_PID=""
LOG_SEQ_PID=""
SEQ_ENODE=""

cleanup() {
    echo -e "\n${RED}[CLEANUP]${NC} Shutting down services..."
    set +e
    if [[ -n "${NODE_PID}" ]]; then
        kill "${NODE_PID}" 2>/dev/null || true
    fi
    if [[ -n "${SEQUENCER_PID}" ]]; then
        kill "${SEQUENCER_PID}" 2>/dev/null || true
    fi
    if [[ -n "${LOG_NODE_PID}" ]]; then
        kill "${LOG_NODE_PID}" 2>/dev/null || true
    fi
    if [[ -n "${LOG_SEQ_PID}" ]]; then
        kill "${LOG_SEQ_PID}" 2>/dev/null || true
    fi
    rm -f "$NODE_PIPE" "$SEQUENCER_PIPE" 2>/dev/null || true
    echo -e "${YELLOW}[LOG]${NC} Node log: ${NODE_LOG}"
    echo -e "${YELLOW}[LOG]${NC} Sequencer log: ${SEQ_LOG}"
}
trap cleanup INT TERM EXIT

require_cmd() {
    if ! command -v "$1" >/dev/null 2>&1; then
        echo -e "${RED}[ERROR]${NC} Missing command: $1"
        exit 1
    fi
}

load_env_if_present() {
    if [[ -f .env ]]; then
        # shellcheck disable=SC2046
        export $(grep -v '^#' .env | xargs) || true
    else
        echo -e "${YELLOW}[WARN]${NC} .env not found; continuing without it."
    fi
}

port_in_use() {
    # args: host port
    local host="$1" port="$2"
    if command -v lsof >/dev/null 2>&1; then
        if lsof -nP -iTCP -sTCP:LISTEN | grep -q "${host}:${port}"; then
            return 0
        else
            return 1
        fi
    elif command -v ss >/dev/null 2>&1; then
        if ss -ltn | awk '{print $4}' | grep -q ":${port}$"; then
            return 0
        else
            return 1
        fi
    else
        if command -v nc >/dev/null 2>&1; then
            if nc -z "$host" "$port" >/dev/null 2>&1; then
                return 0
            else
                return 1
            fi
        fi
        return 1
    fi
}

tcp_open() {
    # args: host port
    local host="$1" port="$2"
    if command -v nc >/dev/null 2>&1; then
        if nc -z "$host" "$port" >/dev/null 2>&1; then
            return 0
        else
            return 1
        fi
    elif command -v ss >/dev/null 2>&1; then
        if ss -ltn | awk '{print $4}' | grep -q ":${port}$"; then
            return 0
        else
            return 1
        fi
    else
        if { echo >/dev/tcp/"$host"/"$port"; } >/dev/null 2>&1; then
            return 0
        else
            return 1
        fi
    fi
}

http_ok_or_known() {
    # args: url
    local url="$1" code
    code="$(curl -sS -o /dev/null -w '%{http_code}' --max-time 2 "$url" || true)"
    if [[ "$code" =~ ^2..$ || "$code" == "404" || "$code" == "405" ]]; then
        return 0
    else
        return 1
    fi
}

jsonrpc_ping() {
    # args: url method
    local url="$1" method="${2:-web3_clientVersion}"
    if curl -fsS --max-time 2 -H 'content-type: application/json' \
        -d "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"$method\",\"params\":[]}" \
        "$url" >/dev/null 2>&1; then
        return 0
    else
        return 1
    fi
}

wait_service() {
    # args: pid host port base_url health_path timeout name [jsonrpc_method]
    local pid="$1" host="$2" port="$3" base_url="$4" health_path="$5" timeout="$6" name="$7" method="${8:-web3_clientVersion}"
    local elapsed=0

    while ((elapsed < timeout)); do
        if ! kill -0 "$pid" 2>/dev/null; then
            echo -e "${RED}[ERROR]${NC} $name exited during startup."
            return 1
        fi

        if tcp_open "$host" "$port"; then
            if http_ok_or_known "${base_url%/}/${health_path#/}"; then
                return 0
            fi
            if jsonrpc_ping "$base_url" "$method"; then
                return 0
            fi
        fi

        sleep "$CHECK_INTERVAL"
        elapsed=$((elapsed + CHECK_INTERVAL))
    done

    echo -e "${RED}[ERROR]${NC} Timeout waiting for $name readiness on ${base_url}."
    return 1
}

extract_sequencer_enode() {
    # Monitor sequencer log for enode URL and extract it
    local timeout=60
    local elapsed=0

    echo -e "${YELLOW}[SEQUENCER]${NC} Waiting for enode URL..."

    while ((elapsed < timeout)); do
        if [[ -f "$SEQ_LOG" ]]; then
            # Look for the enode URL in the log file
            local enode_line
            enode_line=$(grep -o 'enode://[a-fA-F0-9]*@[0-9.]*:[0-9]*' "$SEQ_LOG" 2>/dev/null | head -n1 || true)

            if [[ -n "$enode_line" ]]; then
                SEQ_ENODE="$enode_line"
                echo -e "${BLUE}[SEQUENCER]${NC} Captured enode: ${SEQ_ENODE}"
                return 0
            fi
        fi

        sleep "$CHECK_INTERVAL"
        elapsed=$((elapsed + CHECK_INTERVAL))
    done

    echo -e "${YELLOW}[WARN]${NC} Could not extract sequencer enode URL within timeout, node will start without bootnodes"
    return 1
}

start_loggers() {
    (while read -r line; do echo -e "${GREEN}[NODE]${NC} $line"; done <"$NODE_PIPE") | tee -a "$NODE_LOG" &
    LOG_NODE_PID=$!

    (while read -r line; do
        echo -e "${BLUE}[SEQUENCER]${NC} $line"
        # Check if this line contains an enode and we haven't captured one yet
        if [[ -z "$SEQ_ENODE" && "$line" =~ enode://[a-fA-F0-9]*@[0-9.]*:[0-9]* ]]; then
            SEQ_ENODE=$(echo "$line" | grep -o 'enode://[a-fA-F0-9]*@[0-9.]*:[0-9]*' | head -n1)
            echo -e "${BLUE}[SEQUENCER]${NC} ✅ Captured enode: ${SEQ_ENODE}"
        fi
    done <"$SEQUENCER_PIPE") | tee -a "$SEQ_LOG" &
    LOG_SEQ_PID=$!
}

build_binaries() {
    echo -e "${YELLOW}[BUILD]${NC} Building release binaries…"
    cargo build --release --bins
    echo -e "${GREEN}[BUILD]${NC} Build OK."
}

require_cmd cargo
@@ -196,73 +234,98 @@ export RUST_LOG="${RUST_LOG:-info},mojave=debug"
export RUST_BACKTRACE="${RUST_BACKTRACE:-1}"

if [[ ! -f "$GENESIS" ]]; then
    echo -e "${RED}[ERROR]${NC} Genesis file not found: $GENESIS"
    exit 1
fi

if port_in_use "$SEQ_HOST" "$SEQ_PORT"; then
    echo -e "${RED}[ERROR]${NC} Port in use: ${SEQ_HOST}:${SEQ_PORT}. Stop the process using it and retry."
    exit 1
fi
if port_in_use "$NODE_HOST" "$NODE_PORT"; then
    echo -e "${RED}[ERROR]${NC} Port in use: ${NODE_HOST}:${NODE_PORT}. Stop the process using it and retry."
    exit 1
fi

start_loggers
build_binaries

echo -e "${BLUE}[SEQUENCER]${NC} Starting sequencer…"
(
    set -a
    set +a
    exec cargo run --bin mojave-sequencer -- init \
        --network "$GENESIS" \
        --http.port "$SEQ_PORT" \
        --datadir "$SEQ_DATA_DIR" \
        --private_key "$SEQ_PRIVKEY"
) >"$SEQUENCER_PIPE" 2>&1 &
SEQUENCER_PID=$!

echo -e "${BLUE}[SEQUENCER]${NC} Waiting for sequencer to be ready on ${SEQ_HTTP}…"
if ! wait_service "$SEQUENCER_PID" "$SEQ_HOST" "$SEQ_PORT" "$SEQ_HTTP" "/health" "$SEQ_READY_TIMEOUT" "Sequencer" "web3_clientVersion"; then
    echo -e "${YELLOW}[SEQUENCER LOG TAIL]${NC}"
    tail -n 120 "$SEQ_LOG" || true
    exit 1
fi
echo -e "${BLUE}[SEQUENCER]${NC} Sequencer is ready at ${SEQ_HTTP}"

# Wait a bit more for the enode to appear in logs if we haven't captured it yet
if [[ -z "$SEQ_ENODE" ]]; then
    echo -e "${YELLOW}[SEQUENCER]${NC} Waiting for enode URL to appear in logs..."
    extract_sequencer_enode
fi

echo -e "${GREEN}[NODE]${NC} Starting full node…"

# Build node command with optional bootnodes
node_cmd=(
    cargo run --bin mojave-node -- init
    --network "$GENESIS"
    --datadir "$NODE_DATA_DIR"
    --discovery.port "$NODE_P2P_PORT"
    --p2p.port "$NODE_P2P_PORT"
    --bootnodes "$SEQ_ENODE"
)

if [[ -n "$SEQ_ENODE" ]]; then
    node_cmd+=(--bootnodes "$SEQ_ENODE")
    echo -e "${GREEN}[NODE]${NC} Using bootnode: ${SEQ_ENODE}"
else
    echo -e "${YELLOW}[NODE]${NC} Starting without bootnodes"
fi

(
    set -a
    set +a
    exec "${node_cmd[@]}"
) >"$NODE_PIPE" 2>&1 &
NODE_PID=$!

echo -e "${GREEN}[NODE]${NC} Waiting for full node to be ready on ${NODE_HTTP}…"
if ! wait_service "$NODE_PID" "$NODE_HOST" "$NODE_PORT" "$NODE_HTTP" "/" "$NODE_READY_TIMEOUT" "Full node" "web3_clientVersion"; then
    echo -e "${YELLOW}[NODE LOG TAIL]${NC}"
    tail -n 120 "$NODE_LOG" || true
    exit 1
fi
echo -e "${GREEN}[NODE]${NC} Full node is ready at ${NODE_HTTP}"

echo -e "\n${GREEN}✅ Both services are running!${NC}"
echo -e "   Sequencer: ${SEQ_HTTP}"
echo -e "   Full node: ${NODE_HTTP}"
if [[ -n "$SEQ_ENODE" ]]; then
    echo -e "   Sequencer enode: ${SEQ_ENODE}"
fi
echo -e "   Press ${RED}Ctrl+C${NC} to stop both services…"

# Propagate failure if any process exits
while :; do
    for pid in "$NODE_PID" "$SEQUENCER_PID"; do
        if ! kill -0 "$pid" 2>/dev/null; then
            # process exited, collect its status
            wait "$pid"
            status=$?
            exit $status
        fi
    done
    sleep 0.2
done
echo -e "${RED}[ERROR]${NC} One of the services exited. Showing recent logs…"
echo -e "${YELLOW}[SEQUENCER LOG TAIL]${NC}"
tail -n 120 "$SEQ_LOG" || true
echo -e "${YELLOW}[NODE LOG TAIL]${NC}"
tail -n 120 "$NODE_LOG" || true
exit 1
