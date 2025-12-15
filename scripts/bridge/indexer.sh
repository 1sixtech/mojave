#!/usr/bin/env bash
#
# Manage the bridge UTXO indexer
#
# Usage:
#   ./scripts/bridge/indexer.sh <command> [options]
#
# Commands:
#   start       - Start the indexer
#   stop        - Stop the indexer
#   restart     - Restart the indexer
#   status      - Check indexer status
#   logs        - Show indexer logs (if running in background)
#   install     - Install indexer dependencies
#
# Examples:
#   ./scripts/bridge/indexer.sh start
#   ./scripts/bridge/indexer.sh status
#   ./scripts/bridge/indexer.sh install
#

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
INDEXER_DIR="$(cd "$SCRIPT_DIR/../../contracts/bridge/tools/indexer" && pwd)"
PID_FILE="$INDEXER_DIR/.indexer.pid"

COMMAND="${1:-}"

start_indexer() {
  if [ -f "$PID_FILE" ]; then
    PID=$(cat "$PID_FILE")
    if ps -p "$PID" > /dev/null 2>&1; then
      echo "Indexer is already running (PID: $PID)"
      exit 1
    else
      rm -f "$PID_FILE"
    fi
  fi

  echo "Starting bridge UTXO indexer..."
  cd "$INDEXER_DIR"
  
  if [ ! -f ".env" ]; then
    echo "Warning: .env file not found. Please create one from .env.example"
    exit 1
  fi

  npm start &
  echo $! > "$PID_FILE"
  echo "Indexer started (PID: $(cat $PID_FILE))"
}

stop_indexer() {
  if [ ! -f "$PID_FILE" ]; then
    echo "Indexer is not running (no PID file found)"
    exit 1
  fi

  PID=$(cat "$PID_FILE")
  if ps -p "$PID" > /dev/null 2>&1; then
    echo "Stopping indexer (PID: $PID)..."
    kill "$PID"
    rm -f "$PID_FILE"
    echo "Indexer stopped"
  else
    echo "Indexer is not running (stale PID file)"
    rm -f "$PID_FILE"
  fi
}

status_indexer() {
  if [ -f "$PID_FILE" ]; then
    PID=$(cat "$PID_FILE")
    if ps -p "$PID" > /dev/null 2>&1; then
      echo "Indexer is running (PID: $PID)"
    else
      echo "Indexer is not running (stale PID file)"
    fi
  else
    echo "Indexer is not running"
  fi
}

install_deps() {
  echo "Installing indexer dependencies..."
  cd "$INDEXER_DIR"
  npm install
  echo "Dependencies installed"
}

case "$COMMAND" in
  start)
    start_indexer
    ;;
  stop)
    stop_indexer
    ;;
  restart)
    stop_indexer || true
    sleep 1
    start_indexer
    ;;
  status)
    status_indexer
    ;;
  logs)
    echo "Note: Logs are currently written to stdout/stderr"
    echo "Consider redirecting output when starting:"
    echo "  npm start > indexer.log 2>&1 &"
    ;;
  install)
    install_deps
    ;;
  *)
    echo "Error: Unknown command '$COMMAND'"
    echo ""
    echo "Available commands:"
    echo "  start       - Start the indexer"
    echo "  stop        - Stop the indexer"
    echo "  restart     - Restart the indexer"
    echo "  status      - Check indexer status"
    echo "  logs        - Show indexer logs"
    echo "  install     - Install indexer dependencies"
    exit 1
    ;;
esac
