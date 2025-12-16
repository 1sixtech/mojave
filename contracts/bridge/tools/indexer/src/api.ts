// REST API Server for UTXO Indexer
import { createServer } from 'http';
import type { UtxoIndexer } from './indexer.js';
import type { UtxoPolicy } from './types.js';

export class ApiServer {
    private indexer: UtxoIndexer;
    private port: number;

    constructor(indexer: UtxoIndexer, port: number = 3000) {
        this.indexer = indexer;
        this.port = port;
    }

    start() {
        const server = createServer((req, res) => {
            // CORS headers
            res.setHeader('Access-Control-Allow-Origin', '*');
            res.setHeader('Access-Control-Allow-Methods', 'GET, POST, OPTIONS');
            res.setHeader('Access-Control-Allow-Headers', 'Content-Type');
            res.setHeader('Content-Type', 'application/json');

            // Handle CORS preflight
            if (req.method === 'OPTIONS') {
                res.writeHead(200);
                res.end();
                return;
            }

            try {
                // Support both /api/... and /... paths for backwards compatibility
                const path = req.url?.replace(/^\/api/, '') || '';

                // Handle POST requests
                if (req.method === 'POST' && path === '/utxos/select') {
                    let body = '';
                    req.on('data', (chunk: any) => { body += chunk.toString(); });
                    req.on('end', () => {
                        try {
                            const { amount, policy } = JSON.parse(body);
                            this.handleSelectUtxosPost(req, res, amount, policy || 'LARGEST_FIRST');
                        } catch (error) {
                            res.writeHead(400);
                            res.end(JSON.stringify({ error: 'Invalid JSON body' }));
                        }
                    });
                    return;
                }

                if (path === '/health') {
                    this.handleHealth(req, res);
                } else if (path === '/utxos') {
                    this.handleGetAllUtxos(req, res);
                } else if (path === '/utxos/available') {
                    this.handleGetAvailableUtxos(req, res);
                } else if (path.startsWith('/utxos/')) {
                    const utxoId = path.split('/')[2];
                    this.handleGetUtxo(req, res, utxoId);
                } else if (path.startsWith('/select/')) {
                    const amount = path.split('/')[2];
                    this.handleSelectUtxos(req, res, amount);
                } else if (path === '/stats') {
                    this.handleStats(req, res);
                } else if (path === '/balance') {
                    this.handleBalance(req, res);
                } else {
                    res.writeHead(404);
                    res.end(JSON.stringify({ error: 'Not found' }));
                }
            } catch (error) {
                console.error('API error:', error);
                res.writeHead(500);
                res.end(JSON.stringify({
                    error: error instanceof Error ? error.message : 'Internal server error'
                }));
            }
        });

        server.listen(this.port, () => {
            console.log(`\n🌐 API server running on http://localhost:${this.port}`);
            console.log('📚 Available endpoints:');
            console.log(`   GET  /api/health - Health check`);
            console.log(`   GET  /api/stats - Statistics`);
            console.log(`   GET  /api/utxos - All UTXOs`);
            console.log(`   GET  /api/utxos/available - Available UTXOs`);
            console.log(`   GET  /api/utxos/:id - Get UTXO by ID`);
            console.log(`   GET  /api/select/:amount - Select UTXOs for amount`);
            console.log('');
        });

        return server;
    }

    private handleHealth(req: any, res: any) {
        const stats = this.indexer.getStats();
        const health = {
            status: 'ok' as const,
            uptime: this.indexer.getUptime(),
            stats,
        };

        res.writeHead(200);
        res.end(JSON.stringify(health, null, 2));
    }

    private handleStats(req: any, res: any) {
        const stats = this.indexer.getStats();
        res.writeHead(200);
        res.end(JSON.stringify(stats, null, 2));
    }

    private handleGetAllUtxos(req: any, res: any) {
        const utxos = this.indexer.getAllUtxos();
        const response = {
            count: utxos.length,
            utxos,
        };
        res.writeHead(200);
        res.end(JSON.stringify(response, null, 2));
    }

    private handleGetAvailableUtxos(req: any, res: any) {
        const url = new URL(req.url!, `http://localhost:${this.port}`);
        const policy = (url.searchParams.get('policy') || 'LARGEST_FIRST') as UtxoPolicy;

        const utxos = this.indexer.getAvailableUtxos(policy);
        const response = {
            count: utxos.length,
            utxos,
        };
        res.writeHead(200);
        res.end(JSON.stringify(response, null, 2));
    }

    private handleGetUtxo(req: any, res: any, utxoId: string) {
        const utxo = this.indexer.getUtxo(utxoId);

        if (!utxo) {
            res.writeHead(404);
            res.end(JSON.stringify({ error: 'UTXO not found' }));
            return;
        }

        res.writeHead(200);
        res.end(JSON.stringify(utxo, null, 2));
    }

    private handleSelectUtxos(req: any, res: any, amountStr: string) {
        try {
            const amount = BigInt(amountStr);
            const url = new URL(req.url!, `http://localhost:${this.port}`);
            const policy = (url.searchParams.get('policy') || 'LARGEST_FIRST') as UtxoPolicy;

            this.respondWithSelectedUtxos(res, amount, policy, false);
        } catch (error) {
            res.writeHead(400);
            res.end(JSON.stringify({
                error: error instanceof Error ? error.message : 'Invalid amount'
            }));
        }
    }

    private handleBalance(req: any, res: any) {
        const stats = this.indexer.getStats();
        const balance = {
            total: stats.totalAmount,
            available: stats.availableAmount,
            spent: (BigInt(stats.totalAmount) - BigInt(stats.availableAmount)).toString(),
        };

        res.writeHead(200);
        res.end(JSON.stringify(balance, null, 2));
    }

    private handleSelectUtxosPost(req: any, res: any, amountStr: string, policy: UtxoPolicy) {
        try {
            const amount = BigInt(amountStr);
            this.respondWithSelectedUtxos(res, amount, policy, true);
        } catch (error) {
            res.writeHead(400);
            res.end(JSON.stringify({
                error: error instanceof Error ? error.message : 'Invalid amount'
            }));
        }
    }

    // Shared logic for UTXO selection responses
    private respondWithSelectedUtxos(res: any, amount: bigint, policy: UtxoPolicy, includeCount: boolean) {
        const { utxos, totalAmount } = this.indexer.selectUtxos(amount, policy);

        const response: any = {
            requested: amount.toString(),
            change: (totalAmount - amount).toString(),
        };

        if (includeCount) {
            // POST /utxos/select format
            response.count = utxos.length;
            response.totalSelected = totalAmount.toString();
            response.selected = utxos;
        } else {
            // GET /select/:amount format
            response.selected = totalAmount.toString();
            response.utxos = utxos;
        }

        res.writeHead(200);
        res.end(JSON.stringify(response, null, 2));
    }
}
