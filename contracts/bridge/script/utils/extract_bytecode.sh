#!/bin/bash
#
# Extract Contract Bytecode
#
# This script extracts the deployment bytecode for all contracts
# so they can be deployed without Forge
#

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
OUTPUT_DIR="$PROJECT_ROOT/bytecode"

echo "========================================="
echo "Extracting Contract Bytecode"
echo "========================================="
echo ""

# Create output directory
mkdir -p "$OUTPUT_DIR"

cd "$PROJECT_ROOT"

# Build contracts
echo "[1/4] Building contracts..."
forge build --silent

echo "✓ Build complete"
echo ""

# Extract bytecode for each contract
echo "[2/4] Extracting MockWBTC bytecode..."
WBTC_BYTECODE=$(jq -r '.bytecode.object' out/MockWBTC.sol/MockWBTC.json)
echo "$WBTC_BYTECODE" > "$OUTPUT_DIR/MockWBTC.bin"
echo "  Saved to: $OUTPUT_DIR/MockWBTC.bin"
echo "  Size: $(echo -n "$WBTC_BYTECODE" | wc -c) bytes"

echo ""
echo "[3/4] Extracting MockBtcRelay bytecode..."
RELAY_BYTECODE=$(jq -r '.bytecode.object' out/MockBtcRelay.sol/MockBtcRelay.json)
echo "$RELAY_BYTECODE" > "$OUTPUT_DIR/MockBtcRelay.bin"
echo "  Saved to: $OUTPUT_DIR/MockBtcRelay.bin"
echo "  Size: $(echo -n "$RELAY_BYTECODE" | wc -c) bytes"

echo ""
echo "[4/4] Extracting BridgeGateway bytecode..."
# BridgeGateway requires constructor arguments, so we need the creation code
BRIDGE_BYTECODE=$(jq -r '.bytecode.object' out/BridgeGateway.sol/BridgeGateway.json)
echo "$BRIDGE_BYTECODE" > "$OUTPUT_DIR/BridgeGateway.bin"
echo "  Saved to: $OUTPUT_DIR/BridgeGateway.bin"
echo "  Size: $(echo -n "$BRIDGE_BYTECODE" | wc -c) bytes"

echo ""
echo "========================================="
echo "SUCCESS!"
echo "========================================="
echo ""
echo "Bytecode files saved to: $OUTPUT_DIR"
echo ""
echo "Files:"
ls -lh "$OUTPUT_DIR"/*.bin
echo ""
