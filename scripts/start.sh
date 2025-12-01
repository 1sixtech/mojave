#!/usr/bin/env bash

set -e

# Usage: ./scripts/start.sh [debug|release]
# If omitted, defaults to debug (normal sequencer recipe).
mode="${1:-debug}"
case "$mode" in
  release|--release)
    SEQ_RECIPE="sequencer-release"
    NODE_RECIPE="node-release"
    ;;
  debug|--debug|"")
    SEQ_RECIPE="sequencer"
    NODE_RECIPE="node"
    ;;
  *)
    echo "Usage: $0 [debug|release]" >&2
    exit 1
    ;;
esac

# If CI restored prebuilt binaries, prefer them and skip building
if [[ -n "${BIN_DIR:-}" ]] && [[ -x "$BIN_DIR/mojave-node" ]] && [[ -x "$BIN_DIR/mojave-sequencer" ]]; then
  export SKIP_BUILD=1
  echo "Using prebuilt binaries from: $BIN_DIR (SKIP_BUILD=1)"
fi

# If no BIN_DIR provided, but release binaries are present locally, use them
if [[ -z "${BIN_DIR:-}" ]] && { [[ "$mode" == "release" ]] || [[ "$mode" == "--release" ]]; }; then
  if [[ -x "target/release/mojave-node" ]] && [[ -x "target/release/mojave-sequencer" ]]; then
    export BIN_DIR="target/release"
    export SKIP_BUILD=1
    echo "Detected release binaries in target/release (SKIP_BUILD=1)"
  fi
fi

echo "Starting sequencer using recipe: $SEQ_RECIPE"

# Ensure repo-local log directory exists
mkdir -p .mojave

# Optionally clean up any prior processes and state
just kill-node kill-sequencer clean || true

# Start services and tee their logs to files
just "$SEQ_RECIPE" > .mojave/sequencer.log 2>&1 &
seq_job=$!
sleep 1
just "$NODE_RECIPE" > .mojave/node.log 2>&1 &
node_job=$!

# Stream logs to stdout with prefixes
tail -n +1 -F .mojave/sequencer.log | sed 's/^/[sequencer] /' &
seq_tail=$!
tail -n +1 -F .mojave/node.log | sed 's/^/[full-node] /' &
node_tail=$!

cleanup() {
  printf "\nCaught Ctrl-C, stopping node and sequencer..." >&2
  # Gracefully stop services via just recipes
  just kill-node kill-sequencer || true
  # Stop log tailers (sed processes)
  kill "$seq_tail" "$node_tail" 2>/dev/null || true
  # Kill orphaned tail processes watching our log files
  pkill -f "tail.*\.mojave/sequencer\.log" 2>/dev/null || true
  pkill -f "tail.*\.mojave/node\.log" 2>/dev/null || true
  # Ensure background jobs are not left running
  kill "$seq_job" "$node_job" 2>/dev/null || true
  wait 2>/dev/null || true
}

trap cleanup INT

# Keep running to stream logs until interrupted
wait
