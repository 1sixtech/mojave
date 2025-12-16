// Main entry point for Bridge Indexer
import 'dotenv/config';
import { UtxoIndexer } from './indexer.js';
import { ApiServer } from './api.js';
import type { Address } from 'viem';

async function main() {
    // Load environment variables (support both RPC_URL and PROVIDER_URL for backwards compatibility)
    const RPC_URL = process.env.RPC_URL || process.env.PROVIDER_URL || 'http://localhost:8545';
    const BRIDGE_ADDRESS = process.env.BRIDGE_ADDRESS as Address;
    const PORT = Number(process.env.PORT) || Number(process.env.API_PORT) || 3000;

    // Validate required env vars
    if (!BRIDGE_ADDRESS) {
        console.error('❌ Error: BRIDGE_ADDRESS environment variable is required');
        process.exit(1);
    }

    console.log('════════════════════════════════════════');
    console.log('  Mojave Bridge UTXO Indexer');
    console.log('════════════════════════════════════════');
    console.log('');

    try {
        // Initialize indexer
        const indexer = new UtxoIndexer(RPC_URL, BRIDGE_ADDRESS);

        // Start event listeners
        const cleanup = await indexer.start();

        // Start API server
        const apiServer = new ApiServer(indexer, PORT);
        apiServer.start();

        // Graceful shutdown
        process.on('SIGINT', () => {
            console.log('\n🛑 Shutting down gracefully...');
            cleanup();
            process.exit(0);
        });

        process.on('SIGTERM', () => {
            console.log('\n🛑 Shutting down gracefully...');
            cleanup();
            process.exit(0);
        });

    } catch (error) {
        console.error('❌ Fatal error:', error);
        process.exit(1);
    }
}

// Run
main().catch((error) => {
    console.error('❌ Unhandled error:', error);
    process.exit(1);
});
