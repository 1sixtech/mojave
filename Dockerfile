ARG TARGET_BIN="mojave-sequencer"

FROM rust:1.88 AS builder

RUN apt-get update && apt-get install -y  --no-install-recommends \
	libclang-dev \
	&& rm -rf /var/lib/apt/lists/*

WORKDIR /build

# Target binary name (e.g., mojave-node, mojave-sequencer, mojave-prover)
ARG TARGET_BIN
RUN case "$TARGET_BIN" in \
	mojave-node|mojave-sequencer|mojave-prover) ;; \
	*) echo "Invalid TARGET_BIN=$TARGET_BIN"; exit 1 ;; \
	esac

# Optional build flags
ARG BUILD_FLAGS=""

# Cache deps
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN cargo build --release --bin ${TARGET_BIN} || true

# Build
COPY . .
RUN cargo build --release --bin ${TARGET_BIN} ${BUILD_FLAGS}

# Runtime
FROM debian:bookworm-slim AS runtime-base

ARG TARGET_BIN

RUN apt-get update && apt-get install -y --no-install-recommends \
	libssl3 ca-certificates curl \
	&& rm -rf /var/lib/apt/lists/*

COPY --from=builder /build/target/release/${TARGET_BIN} /usr/local/bin/${TARGET_BIN}

RUN printf '%s\n' \
	'#!/bin/sh' \
	'set -e' \
	'exec "${APP_BIN:-/usr/local/bin/'"${TARGET_BIN}"'}" "$@"' \
	> /usr/local/bin/entrypoint.sh && chmod +x /usr/local/bin/entrypoint.sh
ENTRYPOINT ["/usr/local/bin/entrypoint.sh"]

FROM runtime-base AS mojave-node
COPY data /data
EXPOSE 8545 30304

FROM runtime-base AS mojave-sequencer
COPY data /data
EXPOSE 1739

FROM runtime-base AS mojave-prover

FROM ${TARGET_BIN}
