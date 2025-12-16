#!/usr/bin/env bash
#
# Run end-to-end bridge tests with Bitcoin regtest
#
# Usage:
#   ./scripts/bridge/test-e2e.sh [test_name]
#
# Available tests:
#   incremental         - Incremental signature submission (default, production-like)
#   batch              - Batch signature submission (fast, testing)
#   indexer            - With indexer API integration (full stack)
#
# Examples:
#   ./scripts/bridge/test-e2e.sh                  # Run incremental test
#   ./scripts/bridge/test-e2e.sh incremental      # Run incremental sigs test
#   ./scripts/bridge/test-e2e.sh batch            # Run batch finalization test
#   ./scripts/bridge/test-e2e.sh indexer          # Run with indexer API
#

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BRIDGE_DIR="$(cd "$SCRIPT_DIR/../../contracts/bridge" && pwd)"
MOJAVE_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

# Export MOJAVE_DIR for E2E scripts to find Mojave sequencer
export MOJAVE_DIR="$MOJAVE_ROOT"

TEST_NAME="${1:-incremental}"

case "$TEST_NAME" in
  incremental|incremental_sigs)
    echo "Running E2E test: Incremental signature submission..."
    echo "MOJAVE_DIR: $MOJAVE_DIR"
    cd "$BRIDGE_DIR"
    ./script/e2e/e2e_incremental_sigs.sh
    ;;
  batch|batch_finalization)
    echo "Running E2E test: Batch finalization..."
    echo "MOJAVE_DIR: $MOJAVE_DIR"
    cd "$BRIDGE_DIR"
    ./script/e2e/e2e_batch_finalization.sh
    ;;
  indexer|with_indexer)
    echo "Running E2E test: With indexer API integration..."
    echo "MOJAVE_DIR: $MOJAVE_DIR"
    cd "$BRIDGE_DIR"
    ./script/e2e/e2e_with_indexer_api.sh
    ;;
  *)
    echo "Error: Unknown test '$TEST_NAME'"
    echo ""
    echo "Available tests:"
    echo "  incremental    - Incremental signature submission (production-like)"
    echo "  batch          - Batch finalization (fast testing)"
    echo "  indexer        - With indexer API integration (full stack)"
    exit 1
    ;;
esac
