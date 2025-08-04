#!/usr/bin/env just --justfile

# List all of the available commands.
default:
	just --list

build-mojave:
	cargo build --release

node:
    export $(cat .env | xargs) && \
    cargo run --release --bin mojave-full-node init \
        --network ./test_data/genesis.json \
        --sequencer.address http://0.0.0.0:1739

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

prover:
	export $(cat .env | xargs)

	cargo build --bin mojave

	cargo run --bin mojave -- prover \
		--prover.host 127.0.0.1 \
		--prover.port 3900 \
		# --prover.aligned-mode



generate-key-pair:
	cargo build --bin mojave
	export $(cat .env | xargs) && \
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
	docker build -t 1sixtech/mojave .

docker-run:
	docker run -p 8545:8545 1sixtech/mojave
