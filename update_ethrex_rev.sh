#!/bin/bash

# Script location
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

# Git repo to pull latest hash from
REPO_URL="https://github.com/1sixtech/ethrex"

echo "Fetching latest commit hash from $REPO_URL..."
LATEST_HASH=$(git ls-remote "$REPO_URL" HEAD | cut -f1)

if [[ ! "$LATEST_HASH" =~ ^[a-f0-9]{40}$ ]]; then
    echo "❌ Failed to fetch valid commit hash."
    exit 1
fi

echo "✅ Latest commit hash: $LATEST_HASH"

# Path to root Cargo.toml
CARGO_FILE="$ROOT_DIR/Cargo.toml"

# Backup original file
cp "$CARGO_FILE" "$CARGO_FILE.bak"
echo "📦 Backup created at Cargo.toml.bak"

# Replace all rev fields with new hash
cross_sed() {
    if sed --version 2>/dev/null | grep -q GNU; then
        # GNU sed
        sed -i "$@"
    else
        # BSD sed
        sed -i '' "$@"
    fi
}

cross_sed 's/\(git = "https:\/\/github\.com\/1sixtech\/ethrex".*rev = "\)[^"]*"/\1'"$LATEST_HASH"'"/g' "$CARGO_FILE"

echo "🔄 Updated all rev fields in Cargo.toml to latest commit hash."

# Ask to run cargo update
echo -n "🚀 Run 'cargo update'? (y/n): "
read -r choice
if [[ "$choice" == "y" ]]; then
    cd "$ROOT_DIR" || exit 1
    cargo update
fi
