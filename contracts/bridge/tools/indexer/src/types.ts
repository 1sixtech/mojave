// Type definitions for Bridge Indexer

export interface UTXO {
    utxoId: string;
    txid: string;
    vout: number;
    amount: string; // BigInt as string for JSON serialization
    source: 'DEPOSIT' | 'COLLATERAL';
    spent: boolean;
    createdAt: Date;
    spentInWithdrawal?: string;
    spentAt?: Date;
}

export type UtxoPolicy = 'LARGEST_FIRST' | 'OLDEST_FIRST' | 'SMALLEST_SUFFICIENT';

export interface UtxoStats {
    total: number;
    available: number;
    spent: number;
    totalAmount: string;
    availableAmount: string;
}

export interface HealthStatus {
    status: 'ok' | 'error';
    uptime: number;
    stats: UtxoStats;
    lastBlock?: bigint;
}
