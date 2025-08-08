FROM rust:1.88 AS chef

RUN apt-get update && apt-get install -y  --no-install-recommends \
  build-essential=12.9 \
  libclang-dev=1:14.0-55.7~deb12u1 \
  libc6=2.36-9+deb12u10 \
  libssl-dev=3.0.16-1~deb12u1 \
  ca-certificates=20230311+deb12u1 \
  && rm -rf /var/lib/apt/lists/*
RUN cargo install cargo-chef

WORKDIR /mojave

FROM chef AS planner
COPY crates ./crates
COPY cmd ./cmd
COPY Cargo.* .
# Determine the crates that need to be built from dependencies
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /mojave/recipe.json recipe.json
# Build dependencies only, these remained cached
RUN cargo chef cook --release --recipe-path recipe.json

# Optional build flags
ARG BUILD_FLAGS=""
COPY crates ./crates
COPY cmd ./cmd
COPY Cargo.* ./
RUN cargo build --release $BUILD_FLAGS

FROM ubuntu:24.04
WORKDIR /usr/local/bin

COPY test_data ./test_data
COPY --from=builder mojave/target/release/mojave .
EXPOSE 8545
ENTRYPOINT [ "./mojave" ]
