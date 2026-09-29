import * as kaspa from 'kaspa-wasm';
import { initKaspaWasm } from './init';
import { KASPA_NODE_URL, KASPA_NETWORK, KASPA_PUBLIC_FALLBACK } from '../config/constants';
import { KaspaRpcError, classifyRpcFailure, rpcErrorMessage } from './errors';

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
 *   1. Own node from VITE_KASPA_NODE_URL (e.g. wss://<domain>/kaspa-rpc, Borsh wRPC). It is
 *      verified via getServerInfo: correct network, synced, UTXO index on.
 *   2. Public WASM Resolver — only when no own node is configured, or when
 *      VITE_KASPA_PUBLIC_FALLBACK=true.
 *   3. Error whose code says what actually failed (unreachable / TLS / timeout / wrong
 *      network / not synced / encoding).
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

        // 1. Own node
        if (KASPA_NODE_URL) {
            try {
                const client = new RpcClientClass({
                    url: KASPA_NODE_URL,
                    encoding: kaspa.Encoding.Borsh,
                    networkId: KASPA_NETWORK,
                });
                await client.connect({ strategy: kaspa.ConnectStrategy.Fallback, timeoutDuration: 8000 });
                await verifyNode(client);
                console.log('[kaspa] Connected to configured own node');
                rpcClient = client;
                return client;
            } catch (e) {
                const code = classifyRpcFailure(e);
                console.warn(`[kaspa] Own node failed (${code})`, e);
                if (!KASPA_PUBLIC_FALLBACK) {
                    throw new KaspaRpcError(code, rpcErrorMessage(code, KASPA_NETWORK), {
                        technicalDetail: `[${KASPA_NETWORK}] ${String(e)}`,
                    });
                }
                // Explicitly enabled public fallback: continue with the Resolver.
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

interface ServerInfoLike {
    networkId?: string;
    isSynced?: boolean;
    hasUtxoIndex?: boolean;
}

/** Confirms the node is on the expected network, synced and serves the UTXO index. */
async function verifyNode(client: kaspa.RpcClient): Promise<void> {
    const info = (await client.getServerInfo()) as unknown as ServerInfoLike;
    if (info.networkId && info.networkId !== KASPA_NETWORK) {
        throw new KaspaRpcError('KASPA_WRONG_NETWORK', rpcErrorMessage('KASPA_WRONG_NETWORK', KASPA_NETWORK));
    }
    if (info.isSynced === false) {
        throw new KaspaRpcError('KASPA_NODE_NOT_SYNCED', rpcErrorMessage('KASPA_NODE_NOT_SYNCED', KASPA_NETWORK));
    }
    if (info.hasUtxoIndex === false) {
        throw new KaspaRpcError('KASPA_NO_UTXO_INDEX', rpcErrorMessage('KASPA_NO_UTXO_INDEX', KASPA_NETWORK));
    }
}

export async function disconnectRpc(): Promise<void> {
    if (rpcClient && typeof rpcClient.disconnect === 'function') {
        await rpcClient.disconnect();
        rpcClient = null;
    }
}
