#!/usr/bin/env bash
set -Eeuo pipefail

DATA_DIR="$(pwd)/bitcoin"
CONFIG_DIR="$(pwd)/bitcoin/bitcoin.conf"

# start bitcoin in regtest mode
bitcoind -datadir="$DATA_DIR" -conf="$CONFIG_DIR" -daemonwait

# list wallets if exists load them else create new one
if [ -d "$DATA_DIR/wallets" ]; then
    bitcoin-cli -datadir="$DATA_DIR" -conf="$CONFIG_DIR" loadwallet "mojave-wallet"
else
    bitcoin-cli -datadir="$DATA_DIR" -conf="$CONFIG_DIR" createwallet "mojave-wallet"
fi

# list wallets
bitcoin-cli -datadir="$DATA_DIR" -conf="$CONFIG_DIR" listwallets