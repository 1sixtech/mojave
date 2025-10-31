#!/usr/bin/env bash

set -e

# Ensure repo-local log directory exists
mkdir -p .mojave

# Optionally clean up any prior processes and state
just kill-node kill-sequencer clean || true

# Start services and tee their logs to files
just sequencer > .mojave/sequencer.log 2>&1 &
seq_job=$!
sleep 1
just node > .mojave/node.log 2>&1 &
node_job=$!

# Stream logs to stdout with prefixes
tail -n +1 -F .mojave/sequencer.log | awk '{ print "[sequencer] "$0 }' &
seq_tail=$!
tail -n +1 -F .mojave/node.log | awk '{ print "[full-node] "$0 }' &
node_tail=$!

cleanup() {
  printf "\nCaught Ctrl-C, stopping node and sequencer..." >&2
  # Gracefully stop services via just recipes
  just kill-node kill-sequencer || true
  # Stop log tailers
  kill "$seq_tail" "$node_tail" 2>/dev/null || true
  # Ensure background jobs are not left running
  kill "$seq_job" "$node_job" 2>/dev/null || true
  wait 2>/dev/null || true
}

trap cleanup INT

# Keep running to stream logs until interrupted
wait
