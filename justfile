#!/usr/bin/env just --justfile

# List all of the available commands.

COINBASE_ADDRESS := "0x0007a881CD95B1484fca47615B64803dad620C8d"
COMMITTER_L1_PRIVATE_KEY := "0x385c546456b6a603a1cfcaa9ec9494ba4832da08dd6bcf4de9a71e4a01b74924"
PROOF_COORDINATOR_L1_PRIVATE_KEY := "0x39725efee3fb28614de3bacaffe4cc4bd8c436257e2c8bb887c4b5c4be45e76d"

default:
	just --list

node:
	export $(cat .env | xargs)

	cargo build --bin mojave

	cargo run --bin mojave -- full-node \
		--network ./test_data/genesis.json \
		--l1.bridge-address $(grep ETHREX_WATCHER_BRIDGE_ADDRESS .env | cut -d= -f2) \
		--block-producer.coinbase-address {{COINBASE_ADDRESS}} \
		--committer.l1-private-key {{COMMITTER_L1_PRIVATE_KEY}} \
		--l1.on-chain-proposer-address $(grep ETHREX_COMMITTER_ON_CHAIN_PROPOSER_ADDRESS .env | cut -d= -f2) \
		--proof-coordinator.l1-private-key {{PROOF_COORDINATOR_L1_PRIVATE_KEY}} \
		--sequencer.port 1739 \
		--sequencer.host 127.0.0.1

sequencer:
	export $(cat .env | xargs)

	cargo build --bin mojave

	cargo run --bin mojave -- sequencer \
		--network ./test_data/genesis.json \
		--l1.bridge-address $(grep ETHREX_WATCHER_BRIDGE_ADDRESS .env | cut -d= -f2) \
		--block-producer.coinbase-address {{COINBASE_ADDRESS}} \
		--committer.l1-private-key {{COMMITTER_L1_PRIVATE_KEY}} \
		--l1.on-chain-proposer-address $(grep ETHREX_COMMITTER_ON_CHAIN_PROPOSER_ADDRESS .env | cut -d= -f2) \
		--proof-coordinator.l1-private-key {{PROOF_COORDINATOR_L1_PRIVATE_KEY}}


generate-key-pair:
	export $(cat .env | xargs)

	cargo build --bin mojave

	cargo run --features generate-key-pair --bin mojave generate-key-pair

# Fix some issues
fix flags="":
	cargo fix --allow-staged --all-targets {{flags}}
	cargo clippy --fix --allow-staged --all-targets {{flags}}
	cargo fmt --all

	# requires: cargo install cargo-shear
	cargo shear --fix

	# requires: cargo install cargo-sort
	cargo sort --workspace

	# requires: cargo install cargo-audit
	# cargo audit

	# Update any patch versions
	cargo update

	# cargo install taplo-cli --locked
	taplo fmt

upgrade-ethrex:
	./cmd/update_ethrex_rev.sh

# Upgrade any tooling
upgrade:
	# Update any patch versions
	cargo update

	# Requires: cargo install cargo-upgrades cargo-edit
	cargo upgrade --incompatible

# Build the packages
build:
	cargo build

# Build and serve documentation
doc:
	cargo doc --open --no-deps

# Watch and rebuild documentation on changes
doc-watch:
	cargo watch -x "doc --no-deps"

docker-build:
	docker build -t mojave .

docker-run:
	docker run -p 8545:8545 mojave
