import * as kaspa from 'kaspa-wasm';
import { initKaspaWasm } from './init';
import { KASPA_NODE_URL, KASPA_NETWORK } from '../config/constants';

let rpcClient: kaspa.RpcClient | null = null;

type RpcClientCtor = new (config: {
    url?: string;
    resolver?: kaspa.Resolver;
    encoding: typeof kaspa.Encoding.Borsh;
    networkId: string;
}) => kaspa.RpcClient;

/**
 * Known public Testnet-10 wRPC/Borsh endpoints (fallback list).
 * These are tried in order when the Resolver is unavailable.
 */
const FALLBACK_NODES: readonly string[] = [
    'wss://photon-10.kaspa.red/kaspa/testnet-10/wrpc/borsh',
];

/**
 * Try to resolve a working node URL via the Kaspa Resolver.
 * Uses the Vite proxy (`/kaspa-resolver`) to bypass browser CORS restrictions
 * on the Resolver's HTTP discovery endpoint.
 *
 * Returns a WSS URL string on success, or null if the resolver is down.
 */
async function resolveNodeUrl(): Promise<string | null> {
    const discoveryPath = `/kaspa-resolver/v2/kaspa/${KASPA_NETWORK}/any/wrpc/borsh`;
    try {
        const res = await fetch(discoveryPath, { signal: AbortSignal.timeout(5_000) });
        if (res.ok) {
            // The resolver may return a redirect (followed by fetch) or a URL in the body.
            // The final URL after redirects IS the node endpoint — convert https → wss.
            const finalUrl = res.url;
            if (finalUrl.startsWith('http')) {
                return finalUrl.replace(/^https:/, 'wss:').replace(/^http:/, 'ws:');
            }
            // Or the body itself might be a URL
            const body = await res.text();
            const trimmed = body.trim();
            if (trimmed.startsWith('wss://') || trimmed.startsWith('ws://')) {
                return trimmed;
            }
        }
    } catch (e) {
        console.warn('[kaspa] Resolver discovery failed (network/timeout):', e);
    }
    return null;
}

/**
 * Try connecting an RpcClient to a specific URL.
 * Returns the connected client, or null on failure.
 */
async function tryConnect(url: string): Promise<kaspa.RpcClient | null> {
    try {
        console.log(`[kaspa] Trying node: ${url}`);
        const RpcClientClass = kaspa.RpcClient as unknown as RpcClientCtor;
        const client = new RpcClientClass({
            url,
            encoding: kaspa.Encoding.Borsh,
            networkId: KASPA_NETWORK,
        });
        await client.connect({});
        console.log(`[kaspa] ✅ Connected to ${url}`);
        return client;
    } catch (e) {
        console.warn(`[kaspa] ❌ Failed to connect to ${url}:`, e);
        return null;
    }
}

export async function getRpcClient(): Promise<kaspa.RpcClient> {
    try {
        await initKaspaWasm();

        if (rpcClient) return rpcClient;

        // 1. If a direct URL is configured via env, use it
        if (KASPA_NODE_URL) {
            const client = await tryConnect(KASPA_NODE_URL);
            if (client) {
                rpcClient = client;
                return client;
            }
            console.warn(`[kaspa] Configured node ${KASPA_NODE_URL} unreachable, trying resolver...`);
        }

        // 2. Try the Resolver via Vite proxy (bypasses CORS)
        const resolvedUrl = await resolveNodeUrl();
        if (resolvedUrl) {
            const client = await tryConnect(resolvedUrl);
            if (client) {
                rpcClient = client;
                return client;
            }
        }

        // 3. Try fallback nodes
        for (const fallbackUrl of FALLBACK_NODES) {
            if (fallbackUrl === KASPA_NODE_URL) continue; // already tried
            const client = await tryConnect(fallbackUrl);
            if (client) {
                rpcClient = client;
                return client;
            }
        }

        throw new Error('Keine Kaspa Testnet-10 Node erreichbar. Bitte spaeter erneut versuchen.');
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
