<div align="center">
  <h1>Mojave</h1>
  <img src="assets/header.avif" alt="Mojave Banner" width="600"/>
</div>

<div align="center">
  <a href="https://github.com/1sixtech/mojave/blob/main/LICENSE">
    <img src="https://img.shields.io/badge/license-MIT-blue.svg" alt="License"/>
  </a>
  <a href="https://github.com/1sixtech/mojave/actions/workflows/workflow.push.yml">
    <img src="https://github.com/1sixtech/mojave/actions/workflows/workflow.push.yml/badge.svg" alt="Workflow - Push"/>
  </a>
  <br/><br/>
  <a href="https://github.com/1sixtech/mojave">
	<img src="https://img.shields.io/github/stars/1sixtech/mojave?style=social"/>
  </a>
  <a href="https://twitter.com/intent/follow?screen_name=mojavezk">
    <img src="https://img.shields.io/twitter/follow/mojavezk?style=social" alt="Follow on Twitter"/>
  </a>
  <a href="https://t.me/mojavezk">
    <img src="https://img.shields.io/badge/Telegram-white.svg?logo=telegram" alt="Join Telegram"/>
  </a>
  <a href="https://twitter.com/intent/follow?screen_name=mojavezk">
    <img src="https://img.shields.io/badge/Discord-white.svg?logo=discord" alt="Join Discord"/>
  </a>
</div>

---

## About

**Mojave** is a new layer built on top of Bitcoin. It brings scalability, programmability, and fast transaction speed—without compromising Bitcoin’s core strengths: security and decentralization.

---

## Quickstart

### Clone & Build

```bash
git clone https://github.com/1sixtech/mojave
cd mojave
cargo build --release
```

### Running

```bash
# Node
cargo run --bin mojave-node

# Sequencer
cargo run --bin mojave-sequencer

# Prover
cargo run --bin mojave-prover
```

### Testing

```bash
cargo test --workspace

# e2e tests
bash test_data/tests-e2e.sh
```

---

## Bitcoin Bridge

Mojave includes a trustless Bitcoin bridge that enables secure BTC deposits and withdrawals between Bitcoin and the Mojave L2.

### Features

- **Trustless BTC Deposits** - Uses OP_RETURN commitments and SPV proofs
- **Multi-Signature Withdrawals** - Threshold signatures for secure withdrawals
- **UTXO Tracking** - Efficient indexer for fund management
- **Bitcoin SPV Verification** - Light client implementation (BtcRelay)

### Quick Start

```bash
# Build bridge contracts
./scripts/bridge/build.sh

# Run unit tests
./scripts/bridge/test.sh

# Run E2E tests (requires Bitcoin Core)
./scripts/bridge/test-e2e.sh

# Start UTXO indexer
./scripts/bridge/indexer.sh install
./scripts/bridge/indexer.sh start
```

### Components

- **Smart Contracts** - `contracts/bridge/` - Solidity contracts for bridge logic
- **UTXO Indexer** - `contracts/bridge/tools/indexer/` - TypeScript service with REST API
- **Bridge Types** - `crates/bridge-types/` - Shared Rust types for future integration
- **Scripts** - `scripts/bridge/` - Convenient wrapper scripts

---

## License

Mojave is licensed under the MIT License. See [LICENSE](LICENSE) for details.

## Contributing

PRs are welcome! Read [CONTRIBUTING](CONTRIBUTING.md) to start contributing.

## Acknowledgements

Thanks to the following projects and libraries that made Mojave possible:

- [Bitcoin](https://bitcoin.org/)
- [ethrex](https://github.com/lambdaclass/ethrex)
- [ColliderVM](https://www.collidervm.org/)
