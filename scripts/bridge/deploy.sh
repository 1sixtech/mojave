#!/usr/bin/env bash
#
# Deploy bridge contracts
#
# Usage:
#   ./scripts/bridge/deploy.sh [script_name] [options]
#
# Examples:
#   ./scripts/bridge/deploy.sh DeployBridge.s.sol --rpc-url $RPC_URL --broadcast
#   ./scripts/bridge/deploy.sh                    # Deploy using default script
#

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BRIDGE_DIR="$(cd "$SCRIPT_DIR/../../contracts/bridge" && pwd)"

# Default to DeployBridge.s.sol if no script specified
SCRIPT_NAME="${1:-script/DeployBridge.s.sol}"
shift || true  # Remove first argument if it exists

echo "Deploying bridge contracts using $SCRIPT_NAME..."
cd "$BRIDGE_DIR"
forge script "$SCRIPT_NAME" "$@"
