FROM rust:1.88 AS builder

RUN apt-get update && apt-get install -y  --no-install-recommends \
	libclang-dev \
	&& rm -rf /var/lib/apt/lists/*

WORKDIR /build

# Cache dependencies
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN cargo build --release || true

COPY . .
# Optional build flags
ARG BUILD_FLAGS=""
RUN cargo build --release $BUILD_FLAGS

FROM debian:bullseye-slim

COPY data /usr/local/bin/data
COPY --from=builder /build/target/release/mojave-node /usr/local/bin/mojave-node
COPY --from=builder /build/target/release/mojave-prover /usr/local/bin/mojave-prover
COPY --from=builder /build/target/release/mojave-sequencer /usr/local/bin/mojave-sequencer
EXPOSE 8545
ENTRYPOINT ["/usr/local/bin/mojave-sequencer"]
