# Bridge Scripts

Wrapper scripts for managing Bitcoin bridge contracts, tests, and indexer.

## Overview

These scripts provide a convenient interface to the bridge components located in `contracts/bridge/`. They handle path navigation and provide consistent command-line interfaces.

## Scripts

### build.sh

Build bridge contracts using Forge.

```bash
# Build all contracts
./scripts/bridge/build.sh

# Build with specific options
./scripts/bridge/build.sh --force
./scripts/bridge/build.sh --sizes
```

### test.sh

Run bridge contract unit tests.

```bash
# Run all tests
./scripts/bridge/test.sh

# Run specific test
./scripts/bridge/test.sh --match-test testDeposit

# Run with verbose output
./scripts/bridge/test.sh -vvv

# Run tests matching a pattern
./scripts/bridge/test.sh --match-contract BridgeGateway
```

### deploy.sh

Deploy bridge contracts to a network.

```bash
# Deploy using default script (DeployBridge.s.sol)
./scripts/bridge/deploy.sh --rpc-url $RPC_URL --broadcast --private-key $PRIVATE_KEY

# Deploy using specific script
./scripts/bridge/deploy.sh script/DeployBridge.s.sol --rpc-url $RPC_URL --broadcast

# Dry run (simulate deployment)
./scripts/bridge/deploy.sh --rpc-url $RPC_URL
```

### test-e2e.sh

Run end-to-end integration tests with Bitcoin regtest network.

```bash
# Run default E2E test (incremental signatures)
./scripts/bridge/test-e2e.sh

# Run specific E2E test
./scripts/bridge/test-e2e.sh incremental_sigs
./scripts/bridge/test-e2e.sh utxo_indexer
```

**Available tests:**
- `incremental_sigs` - Full cycle with incremental signatures (default)
- `utxo_indexer` - Full cycle with UTXO indexer integration

### indexer.sh

Manage the bridge UTXO indexer service.

```bash
# Install dependencies
./scripts/bridge/indexer.sh install

# Start indexer
./scripts/bridge/indexer.sh start

# Check status
./scripts/bridge/indexer.sh status

# Stop indexer
./scripts/bridge/indexer.sh stop

# Restart indexer
./scripts/bridge/indexer.sh restart
```

**Note:** The indexer requires a `.env` file in `contracts/bridge/tools/indexer/`. Copy from `.env.example` and configure:
```bash
cp contracts/bridge/tools/indexer/.env.example contracts/bridge/tools/indexer/.env
# Edit .env with your configuration
```

## Directory Structure

```
mojave/
├── scripts/bridge/              # Wrapper scripts (this directory)
│   ├── build.sh
│   ├── test.sh
│   ├── deploy.sh
│   ├── test-e2e.sh
│   └── indexer.sh
└── contracts/bridge/            # Bridge implementation
    ├── src/                     # Solidity contracts
    ├── test/                    # Unit tests
    ├── script/                  # Forge scripts (deployment, E2E tests)
    │   └── flow/               # E2E test flows
    └── tools/indexer/          # TypeScript UTXO indexer
```

## Development Workflow

### 1. Build and Test

```bash
# Build contracts
./scripts/bridge/build.sh

# Run unit tests
./scripts/bridge/test.sh

# Run unit tests with gas reports
./scripts/bridge/test.sh --gas-report
```

### 2. Run E2E Tests

```bash
# Full integration test with Bitcoin regtest
./scripts/bridge/test-e2e.sh incremental_sigs
```

### 3. Deploy to Network

```bash
# Set environment variables
export RPC_URL=http://localhost:8545
export PRIVATE_KEY=0x...

# Deploy contracts
./scripts/bridge/deploy.sh --rpc-url $RPC_URL --broadcast --private-key $PRIVATE_KEY
```

### 4. Run Indexer

```bash
# Configure indexer
cp contracts/bridge/tools/indexer/.env.example contracts/bridge/tools/indexer/.env
# Edit .env

# Install dependencies (first time only)
./scripts/bridge/indexer.sh install

# Start indexer
./scripts/bridge/indexer.sh start

# Check status
./scripts/bridge/indexer.sh status
```

## Advanced Usage

### Custom Forge Scripts

To run other Forge scripts in the bridge directory:

```bash
cd contracts/bridge
forge script script/YourScript.s.sol --rpc-url $RPC_URL --broadcast
```

### Direct Shell Scripts

The original shell scripts in `contracts/bridge/script/flow/` can be run directly:

```bash
cd contracts/bridge
./script/flow/bitcoin_deposit.sh
./script/flow/fetch_bitcoin_headers.sh
```

### Manual Indexer Management

For development, you can run the indexer directly:

```bash
cd contracts/bridge/tools/indexer
npm install
npm start
```

## CI/CD Integration

These scripts are designed to be used in CI/CD pipelines:

```yaml
# Example GitHub Actions workflow
- name: Build contracts
  run: ./scripts/bridge/build.sh

- name: Run tests
  run: ./scripts/bridge/test.sh

- name: Run E2E tests
  run: ./scripts/bridge/test-e2e.sh
```

## Troubleshooting

### "Indexer is not running"

Make sure you have:
1. Created the `.env` file with correct configuration
2. Installed dependencies: `./scripts/bridge/indexer.sh install`
3. Started the indexer: `./scripts/bridge/indexer.sh start`

### "forge: command not found"

Install Foundry:
```bash
curl -L https://foundry.paradigm.xyz | bash
foundryup
```

### E2E tests fail

E2E tests require:
1. Bitcoin Core (for regtest)
2. Running Ethereum node (Anvil or similar)
3. Proper environment variables set

See `contracts/bridge/README.md` for detailed setup instructions.

## See Also

- [Bridge Contracts Documentation](../../contracts/bridge/README.md)
- [Indexer Documentation](../../contracts/bridge/tools/indexer/README.md)
- [Architecture Overview](../../contracts/bridge/ARCHITECTURE.md)
