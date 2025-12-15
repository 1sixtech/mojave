// UTXO Indexer - Listens to Bridge contract events
import {
    createPublicClient,
    http,
    parseAbiItem,
    type Address,
    type Hash,
    type PublicClient,
} from 'viem';
import type { UTXO, UtxoPolicy, UtxoStats } from './types.js';

// Event signatures (defined once to avoid duplication)
const UTXO_REGISTERED_EVENT = parseAbiItem(
    'event UtxoRegistered(bytes32 indexed utxoId, bytes32 indexed txid, uint32 vout, uint256 amount, uint8 indexed source, uint256 timestamp)'
);

const UTXO_SPENT_EVENT = parseAbiItem(
    'event UtxoSpent(bytes32 indexed utxoId, bytes32 indexed wid, uint256 timestamp)'
);

interface UtxoRegisteredArgs {
    utxoId: Hash;
    txid: Hash;
    vout: number;
    amount: bigint;
    source: number;
    timestamp: bigint;
}

interface UtxoSpentArgs {
    utxoId: Hash;
    wid: Hash;
    timestamp: bigint;
}

export class UtxoIndexer {
    private utxos = new Map<string, UTXO>();
    private client: PublicClient;
    private bridgeAddress: Address;
    private startTime = Date.now();

    constructor(rpcUrl: string, bridgeAddress: Address) {
        this.bridgeAddress = bridgeAddress;
        this.client = createPublicClient({
            transport: http(rpcUrl),
        });
    }

    async start() {
        console.log('🚀 UTXO Indexer starting...');
        console.log(`📡 Monitoring bridge at ${this.bridgeAddress}`);
        console.log(`🔗 RPC: ${this.client.transport.url}`);

        // First, fetch all past events
        console.log('📜 Syncing past events...');
        await this.syncPastEvents();
        console.log(`✅ Synced ${this.utxos.size} UTXO(s) from past events`);

        // Watch UtxoRegistered events
        const unwatch1 = this.client.watchEvent({
            address: this.bridgeAddress,
            event: UTXO_REGISTERED_EVENT,
            onLogs: (logs) => this.handleUtxoRegistered(logs),
        });

        // Watch UtxoSpent events
        const unwatch2 = this.client.watchEvent({
            address: this.bridgeAddress,
            event: UTXO_SPENT_EVENT,
            onLogs: (logs) => this.handleUtxoSpent(logs),
        });

        console.log('✅ Event listeners active');

        // Return cleanup function
        return () => {
            unwatch1();
            unwatch2();
        };
    }

    private async syncPastEvents() {
        try {
            const currentBlock = await this.client.getBlockNumber();

            // Fetch all events in parallel
            const [registeredLogs, spentLogs] = await Promise.all([
                this.client.getLogs({
                    address: this.bridgeAddress,
                    event: UTXO_REGISTERED_EVENT,
                    fromBlock: 0n,
                    toBlock: currentBlock,
                }),
                this.client.getLogs({
                    address: this.bridgeAddress,
                    event: UTXO_SPENT_EVENT,
                    fromBlock: 0n,
                    toBlock: currentBlock,
                })
            ]);

            // Process registered events first, then spent events
            this.handleUtxoRegistered(registeredLogs);
            this.handleUtxoSpent(spentLogs);
        } catch (error) {
            console.error('Error syncing past events:', error);
            throw error;
        }
    }

    private handleUtxoRegistered(logs: any[]) {
        for (const log of logs) {
            try {
                const { utxoId, txid, vout, amount, source, timestamp } = log.args as UtxoRegisteredArgs;

                const utxo: UTXO = {
                    utxoId,
                    txid,
                    vout: Number(vout),
                    amount: amount.toString(),
                    source: source === 1 ? 'DEPOSIT' : 'COLLATERAL',
                    spent: false,
                    createdAt: new Date(Number(timestamp) * 1000),
                };

                this.utxos.set(utxoId, utxo);

                console.log(
                    `✅ UTXO registered: ${utxoId.slice(0, 10)}... ` +
                    `(${utxo.source}, ${(Number(amount) / 1e8).toFixed(8)} BTC)`
                );
            } catch (error) {
                console.error('Error handling UtxoRegistered:', error);
            }
        }
    }

    private handleUtxoSpent(logs: any[]) {
        for (const log of logs) {
            try {
                const { utxoId, wid, timestamp } = log.args as UtxoSpentArgs;

                const utxo = this.utxos.get(utxoId);
                if (utxo) {
                    utxo.spent = true;
                    utxo.spentInWithdrawal = wid;
                    utxo.spentAt = new Date(Number(timestamp) * 1000);

                    console.log(
                        `❌ UTXO spent: ${utxoId.slice(0, 10)}... ` +
                        `in withdrawal ${wid.slice(0, 10)}...`
                    );
                }
            } catch (error) {
                console.error('Error handling UtxoSpent:', error);
            }
        }
    }

    // Get all UTXOs
    getAllUtxos(): UTXO[] {
        return Array.from(this.utxos.values());
    }

    // Get available (unspent) UTXOs
    getAvailableUtxos(policy: UtxoPolicy = 'LARGEST_FIRST'): UTXO[] {
        const available = Array.from(this.utxos.values()).filter(u => !u.spent);

        switch (policy) {
            case 'LARGEST_FIRST':
                return available.sort((a, b) =>
                    Number(BigInt(b.amount) - BigInt(a.amount))
                );
            case 'OLDEST_FIRST':
                return available.sort((a, b) =>
                    a.createdAt.getTime() - b.createdAt.getTime()
                );
            case 'SMALLEST_SUFFICIENT':
                return available.sort((a, b) =>
                    Number(BigInt(a.amount) - BigInt(b.amount))
                );
            default:
                return available;
        }
    }

    // Select UTXOs for a withdrawal amount
    selectUtxos(
        targetAmount: bigint,
        policy: UtxoPolicy = 'LARGEST_FIRST'
    ): { utxos: UTXO[], totalAmount: bigint } {
        const available = this.getAvailableUtxos(policy);
        const selected: UTXO[] = [];
        let totalAmount = 0n;

        for (const utxo of available) {
            if (totalAmount >= targetAmount) break;
            selected.push(utxo);
            totalAmount += BigInt(utxo.amount);
        }

        if (totalAmount < targetAmount) {
            throw new Error(
                `Insufficient UTXOs: need ${targetAmount}, have ${totalAmount}`
            );
        }

        return { utxos: selected, totalAmount };
    }

    // Get UTXO by ID
    getUtxo(utxoId: string): UTXO | undefined {
        return this.utxos.get(utxoId);
    }

    // Get statistics
    getStats(): UtxoStats {
        const all = this.getAllUtxos();
        const available = all.filter(u => !u.spent);

        const totalAmount = all.reduce(
            (sum, u) => sum + BigInt(u.amount),
            0n
        );
        const availableAmount = available.reduce(
            (sum, u) => sum + BigInt(u.amount),
            0n
        );

        return {
            total: all.length,
            available: available.length,
            spent: all.length - available.length,
            totalAmount: totalAmount.toString(),
            availableAmount: availableAmount.toString(),
        };
    }

    // Get uptime
    getUptime(): number {
        return Date.now() - this.startTime;
    }
}
