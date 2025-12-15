# UTXO Indexer

TypeScript-based API server for indexing and managing UTXOs from the Mojave Bridge contract.

## Features

- Real-time event monitoring for UTXO registration and spending
- RESTful API for querying UTXOs
- Multiple UTXO selection policies (LARGEST_FIRST, OLDEST_FIRST, SMALLEST_SUFFICIENT)
- Health check and statistics endpoints

## Setup

1. Install dependencies:
```bash
npm install
```

2. Configure environment:
```bash
cp .env.example .env
# Edit .env with your values
```

3. Run the indexer:
```bash
npm start
```

For development with hot reload:
```bash
npm run dev
```

## API Endpoints

### Health & Stats
- `GET /api/health` - Server health status
- `GET /api/stats` - UTXO statistics

### UTXO Queries
- `GET /api/utxos` - All UTXOs
- `GET /api/utxos/available` - Available (unspent) UTXOs
- `GET /api/utxos/:id` - Get specific UTXO by ID

### UTXO Selection
- `GET /api/select/:amount?policy=<policy>` - Select UTXOs for amount
  - Policies: `LARGEST_FIRST`, `OLDEST_FIRST`, `SMALLEST_SUFFICIENT`
  - Example: `/api/select/5000000?policy=SMALLEST_SUFFICIENT`

## Architecture

This is a temporary TypeScript tool that will eventually be migrated to the Mojave sequencer.

```
indexer/
├── src/
│   ├── types.ts       # Type definitions
│   ├── indexer.ts     # UTXO indexer logic
│   ├── api.ts         # REST API server
│   └── index.ts       # Entry point
├── package.json
├── tsconfig.json
└── .env
```

## Migration Plan

This indexer will be moved to the Mojave monorepo as:
```
mojave/contracts/bridge/tools/indexer/
```

Eventually, the indexing functionality will be integrated into the Mojave sequencer.
