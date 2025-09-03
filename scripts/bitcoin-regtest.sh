#!/bin/bash
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Bitcoin configuration
BITCOIN_DATA_DIR="/bitcoin"
BITCOIN_CONFIG_DIR="/bitcoin/bitcoin.conf"
WALLETNAME=mojave-wallet

bitcoind -daemon \
    -conf=$BITCOIN_CONFIG_DIR \
    -datadir=$BITCOIN_DATA_DIR
BTC="bitcoin-cli -conf=$BITCOIN_CONFIG_DIR -datadir=$BITCOIN_DATA_DIR"

# Wait until RPC is ready
sleep 5
while ! $BTC getblockchaininfo > /dev/null 2>&1; do
  echo "Waiting for bitcoind..."
  sleep 2
done


if [ -d "$BITCOIN_DATA_DIR/wallets" ]; then
    $BTC loadwallet "$WALLETNAME"
else
    $BTC createwallet "$WALLETNAME"
fi

BITCOIN_ADDRESS=$($BTC getnewaddress "mojave-address")

# Generate 101 blocks and pay block rewards of 50 bitcoins
$BTC generatetoaddress 101 "$BITCOIN_ADDRESS"

# Verify balance
BITCOIN_BALANCE=$($BTC getbalance)
if [ "$BITCOIN_BALANCE" != "50.00000000" ]; then
    echo -e "${RED}[ERROR]${NC} Bitcoin balance is not 50.00000000, got: $BITCOIN_BALANCE"
    exit 1
fi

echo "Bitcoin setup complete! Balance: $BITCOIN_BALANCE BTC"

# Install watch
apt-get update
apt-get install -y procps
# A terminal-based program (like watch, top, less, etc.) runs in an environment, TERM environment variable should be set
export TERM=xterm
watch -n 2 "$BTC generatetoaddress 1 $BITCOIN_ADDRESS"