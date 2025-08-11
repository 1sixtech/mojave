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
    export $(cat .env | xargs) && \
    cargo run --release --bin mojave-sequencer init \
        --network ./test_data/genesis.json \
        --http.port 1739 \
        --full_node.addresses http://0.0.0.0:8545

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
