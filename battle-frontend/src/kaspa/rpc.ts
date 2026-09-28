import * as kaspa from 'kaspa-wasm';
import { initKaspaWasm } from './init';
import { KASPA_NODE_URL, KASPA_NETWORK } from '../config/constants';
import { KaspaRpcError } from './errors';

let rpcClient: kaspa.RpcClient | null = null;

type RpcClientCtor = new (config: {
    url?: string;
    resolver?: kaspa.Resolver;
    encoding: typeof kaspa.Encoding.Borsh;
    networkId: string;
}) => kaspa.RpcClient;

/**
 * Get or create a connected RPC client.
 *
 * Connection strategy (in order):
 *   1. Direct URL from VITE_KASPA_NODE_URL (if set)
 *   2. WASM SDK built-in Resolver (works in browser for WebSocket — no CORS issue)
 *   3. Error with clear message
 *
 * The WASM Resolver (`new kaspa.Resolver()`) handles node discovery internally
 * via WebSocket, which is NOT subject to browser CORS restrictions.
 * The old HTTP-based resolver proxy (/kaspa-resolver) has been removed —
 * it only worked with the Vite dev server and paul/alex.kaspa.red are unreliable.
 */
export async function getRpcClient(): Promise<kaspa.RpcClient> {
    try {
        await initKaspaWasm();

        if (rpcClient) return rpcClient;

        const RpcClientClass = kaspa.RpcClient as unknown as RpcClientCtor;

        // 1. Direct URL from environment
        if (KASPA_NODE_URL) {
            console.log(`[kaspa] Connecting to configured node: ${KASPA_NODE_URL}`);
            try {
                const client = new RpcClientClass({
                    url: KASPA_NODE_URL,
                    encoding: kaspa.Encoding.Borsh,
                    networkId: KASPA_NETWORK,
                });
                await client.connect({ strategy: kaspa.ConnectStrategy.Fallback, timeoutDuration: 8000 });
                console.log(`[kaspa] ✅ Connected to ${KASPA_NODE_URL}`);
                rpcClient = client;
                return client;
            } catch (e) {
                console.warn(`[kaspa] ❌ Configured node unreachable: ${KASPA_NODE_URL}`, e);
                // Fall through to Resolver
            }
        }

        // 2. WASM SDK built-in Resolver — discovers nodes automatically
        console.log(`[kaspa] Using WASM Resolver for network: ${KASPA_NETWORK}`);
        try {
            const resolver = new kaspa.Resolver();
            const client = new RpcClientClass({
                resolver,
                encoding: kaspa.Encoding.Borsh,
                networkId: KASPA_NETWORK,
            });
            await client.connect({ strategy: kaspa.ConnectStrategy.Fallback, timeoutDuration: 10000 });
            console.log(`[kaspa] ✅ Connected via Resolver`);
            rpcClient = client;
            return client;
        } catch (resolverError) {
            console.error(`[kaspa] ❌ Resolver failed for ${KASPA_NETWORK}:`, resolverError);
            throw new KaspaRpcError(
                'KASPA_RPC_UNAVAILABLE',
                `Keine ${KASPA_NETWORK} Node erreichbar. ` +
                `Der Kaspa Resolver konnte keine verfügbare Node finden. ` +
                `Bitte später erneut versuchen oder eine direkte Node-URL konfigurieren.`,
                { technicalDetail: `[${KASPA_NETWORK}] ${String(resolverError)}` }
            );
        }
    } catch (error) {
        console.error('[kaspa] RPC initialization failed:', error);
        rpcClient = null;
        throw error;
    }
}

export async function disconnectRpc(): Promise<void> {
    if (rpcClient && typeof rpcClient.disconnect === 'function') {
        await rpcClient.disconnect();
        rpcClient = null;
    }
}
