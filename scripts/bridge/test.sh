#!/usr/bin/env bash
#
# Run bridge contract unit tests
#
# Usage:
#   ./scripts/bridge/test.sh [options]
#
# Examples:
#   ./scripts/bridge/test.sh                    # Run all tests
#   ./scripts/bridge/test.sh --match-test testDeposit  # Run specific test
#   ./scripts/bridge/test.sh -vvv              # Run with verbose output
#

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BRIDGE_DIR="$(cd "$SCRIPT_DIR/../../contracts/bridge" && pwd)"

echo "Running bridge contract tests..."
cd "$BRIDGE_DIR"
forge test "$@"
