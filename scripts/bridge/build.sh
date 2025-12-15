#!/usr/bin/env bash
#
# Build bridge contracts
#
# Usage:
#   ./scripts/bridge/build.sh [options]
#
# Options are forwarded to 'forge build'
#

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BRIDGE_DIR="$(cd "$SCRIPT_DIR/../../contracts/bridge" && pwd)"

echo "Building bridge contracts..."
cd "$BRIDGE_DIR"
forge build "$@"
