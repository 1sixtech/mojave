# Bitcoin Bridge Contracts

Smart contracts for Mojave's trustless Bitcoin bridge, enabling secure BTC deposits and withdrawals between Bitcoin and Mojave L2.

## Overview

The Mojave Bitcoin Bridge provides:
- **Trustless BTC deposits** via OP_RETURN commitment proofs
- **Multi-signature withdrawals** with threshold signatures
- **UTXO tracking** for efficient fund management
- **Bitcoin SPV verification** through BtcRelay

## Architecture

### Core Components

1. **BridgeGateway** (`src/BridgeGateway.sol`)
   - Main bridge contract managing deposits and withdrawals
   - Handles envelope verification and UTXO management
   - Integrates with BtcRelay for Bitcoin header verification

2. **BtcRelay** (`src/relay/BtcRelay.sol`)
   - Bitcoin SPV light client implementation
   - Stores Bitcoin block headers and validates merkle proofs
   - Provides header verification for deposit proofs

3. **WBTC Token** (`src/token/WBTC.sol`)
   - ERC20 wrapped Bitcoin token on Mojave L2
   - Minted on successful BTC deposits
   - Burned on BTC withdrawals

### Supporting Tools

- **UTXO Indexer** (`tools/indexer/`)
  - TypeScript service tracking bridge UTXO state
  - REST API for querying available UTXOs
  - Monitors deposit and withdrawal events

## Quick Start

### Prerequisites

- [Foundry](https://book.getfoundry.sh/getting-started/installation) (forge, cast, anvil)
- [Bitcoin Core](https://bitcoin.org/en/download) (for regtest in E2E tests)
- Node.js 18+ (for indexer)

### Build

From the repository root:
```bash
./scripts/bridge/build.sh
```

Or directly:
```bash
cd contracts/bridge
forge build
```

### Test

Run unit tests:
```bash
./scripts/bridge/test.sh
```

Run specific test:
```bash
./scripts/bridge/test.sh --match-test testDeposit
```

Run with gas reports:
```bash
./scripts/bridge/test.sh --gas-report
```

### Deploy

```bash
# Set environment variables
export RPC_URL=http://localhost:8545
export PRIVATE_KEY=0x...

# Deploy contracts
./scripts/bridge/deploy.sh --rpc-url $RPC_URL --broadcast --private-key $PRIVATE_KEY
```

## End-to-End Testing

The bridge includes comprehensive E2E tests that run against real Bitcoin regtest network:

### Test Flows

1. **Incremental Signatures Flow** (recommended)
```bash
./scripts/bridge/test-e2e.sh incremental
```

This test:
- Sets up Bitcoin regtest network
- Deploys bridge contracts on local EVM
- Creates Bitcoin deposit with OP_RETURN commitment
- Mines blocks and generates merkle proof
- Submits proof to bridge contract
- Verifies WBTC minting
- Initiates withdrawal
- Builds and signs PSBT
- Completes withdrawal cycle

2. **Batch Flow**
```bash
./scripts/bridge/test-e2e.sh batch
```

3. **UTXO Indexer Flow**
```bash
./scripts/bridge/test-e2e.sh indexer
```

This test includes:
- All steps from incremental signatures flow
- Running UTXO indexer service
- Querying indexer API for UTXO selection
- Withdrawal using indexer-selected UTXOs

### Manual E2E Testing

For development, you can run individual scripts:

```bash
cd contracts/bridge

# 1. Start Bitcoin regtest
./script/flow/bitcoin_deposit.sh

# 2. Fetch Bitcoin headers
./script/flow/fetch_bitcoin_headers.sh

# 3. Submit headers to BtcRelay
forge script script/flow/SubmitBitcoinHeaders.s.sol --broadcast

# 4. Calculate deposit envelope
forge script script/flow/Step1_UserCalculatesEnvelope.s.sol

# 5. Submit deposit proof
forge script script/flow/Step3_OperatorSubmitsProof.s.sol --broadcast

# 6. Request withdrawal
forge script script/flow/Step4_UserRequestsWithdrawal.s.sol --broadcast
```

## UTXO Indexer

The indexer tracks bridge UTXO state and provides REST API for withdrawal UTXO selection.

### Setup

```bash
# Install dependencies
./scripts/bridge/indexer.sh install

# Configure
cp contracts/bridge/tools/indexer/.env.example contracts/bridge/tools/indexer/.env
# Edit .env with your configuration
```

### Running

```bash
# Start indexer
./scripts/bridge/indexer.sh start

# Check status
./scripts/bridge/indexer.sh status

# Stop indexer
./scripts/bridge/indexer.sh stop
```

### API Endpoints

- `GET /health` - Health check
- `GET /stats` - UTXO statistics (total count, confirmed, pending, spent)
- `GET /utxos` - List all UTXOs with filters
- `GET /balance/:address` - Get balance for address
- `POST /utxos/select` - Select UTXOs for withdrawal amount

See [indexer documentation](tools/indexer/README.md) for details.

## Security

### Audits

- [ ] Internal audit - Pending
- [ ] External audit - Planned Q1 2025

### Bug Bounty

We will launch a bug bounty program after mainnet deployment.

## Development

### Project Structure

```
contracts/bridge/
├── src/                    # Solidity contracts
│   ├── BridgeGateway.sol  # Main bridge contract
│   ├── relay/
│   │   └── BtcRelay.sol   # Bitcoin SPV relay
│   ├── token/
│   │   └── WBTC.sol       # Wrapped BTC token
│   └── mocks/             # Mock contracts for testing
├── test/                   # Foundry tests
│   ├── BridgeGateway.t.sol
│   ├── BtcRelay.t.sol
│   └── GasCostAnalysis.t.sol
├── script/                 # Deployment & E2E scripts
│   ├── DeployBridge.s.sol
│   └── flow/              # E2E test flows
└── tools/
    └── indexer/           # TypeScript UTXO indexer
```

### Adding New Features

1. Write contract code in `src/`
2. Add unit tests in `test/`
3. Run tests: `./scripts/bridge/test.sh`
4. Add E2E test flow in `script/flow/`
5. Update indexer if needed

### Gas Optimization

Run gas analysis:
```bash
forge test --gas-report
```

Run specific gas analysis test:
```bash
forge test --match-contract GasCostAnalysis --gas-report
```

## Troubleshooting

### "Library not found" errors

Make sure git submodules are initialized:
```bash
git submodule update --init --recursive
```

### E2E tests fail

Check requirements:
- Bitcoin Core installed and `bitcoin-cli` in PATH
- Port 18443 (regtest) available
- Port 8545 (anvil) available
- Foundry up to date: `foundryup`

### Indexer connection issues

Verify:
- RPC URL is correct in `.env`
- Contract addresses are deployed
- Network is accessible
- Events are being emitted (check with cast)

## Resources

- [Architecture Documentation](../../ARCHITECTURE.md) (if exists in main repo)
- [Indexer README](tools/indexer/README.md)