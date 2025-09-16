#!/usr/bin/env just --justfile

home-dir := env_var('HOME')
current-dir := `pwd`

# List all of the available commands.
default:
	just --list

build-mojave:
	cargo build --release

clean:
	killall mojave-node mojave-sequencer || true
	rm -rf {{home-dir}}/.mojave/

# Run both node and sequencer in parallel, with sequencer waiting for node
full: clean
	./scripts/start.sh

node:
    export $(cat .env | xargs) && \
    cargo run --release --bin mojave-node init \
        --network {{current-dir}}/data/testnet-genesis.json

sequencer:
    export $(cat .env | xargs) && \
    cargo run --release --bin mojave-sequencer init \
        --http.port 1739 \
        --private_key 0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa \
        --network {{current-dir}}/data/testnet-genesis.json

# Run bitcoin regtest in docker
bitcoin-start:
	docker run -d --name bitcoin-regtest \
	 --restart unless-stopped \
	 -p 18443:18443 \
	 -p 18444:18444 \
	 -v {{current-dir}}/bitcoin/:/bitcoin \
	 -v {{current-dir}}/bitcoin/bitcoin.conf:/bitcoin/bitcoin.conf \
	 -v {{current-dir}}/scripts/bitcoin-regtest.sh:/usr/local/bin/bitcoin-regtest.sh \
	 --entrypoint /bin/bash \
	 ruimarinho/bitcoin-core \
	 /usr/local/bin/bitcoin-regtest.sh

bitcoin-stop:
	docker stop bitcoin-regtest

bitcoin-clean:
	docker rm -f bitcoin-regtest
	rm -rf bitcoin/regtest

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
	cargo sort --workspace -g

	# requires: cargo install cargo-audit
	# cargo audit

	# Update any patch versions
	cargo update

	# cargo install taplo-cli --locked
	taplo fmt

upgrade-ethrex:
	./scripts/update_ethrex_rev.sh

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

image-prefix := "1sixtech"

# Build the docker image for a specific binary
# Binary name should be one of: mojave-node, mojave-sequencer, mojave-prover
docker-build bin:
	role="{{bin}}"; \
	role="${role#mojave-}"; \
	docker build \
	  -f "docker/Dockerfile.$role" \
	  -t "{{image-prefix}}/{{bin}}" \
	  --build-arg "TARGET_BIN={{bin}}" \
	  .

docker-run bin *ARGS:
	docker run -p 8545:8545 -p 1739:1739 -p 30304:30304 "{{image-prefix}}/{{bin}}" {{ARGS}}

test: clean
	bash tests/tests-e2e.sh
