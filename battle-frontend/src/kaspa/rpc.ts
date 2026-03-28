import * as kaspa from 'kaspa-wasm';
import { initKaspaWasm } from './init';
import { KASPA_NODE_URL, KASPA_NETWORK } from '../config/constants';

let rpcClient: kaspa.RpcClient | null = null;

export async function getRpcClient(): Promise<kaspa.RpcClient> {
    try {
        console.log('[kaspa] Initializing WASM...');
        await initKaspaWasm();

        if (rpcClient) return rpcClient;

        const rpcConfig = KASPA_NODE_URL
            ? {
                url: KASPA_NODE_URL,
                encoding: kaspa.Encoding.Borsh,
                networkId: KASPA_NETWORK,
            }
            : {
                resolver: new kaspa.Resolver(),
                encoding: kaspa.Encoding.Borsh,
                networkId: KASPA_NETWORK,
            };

        console.log(
            KASPA_NODE_URL
                ? `[kaspa] Creating RpcClient for ${KASPA_NODE_URL}`
                : `[kaspa] Creating RpcClient via Resolver for ${KASPA_NETWORK}`
        );

        rpcClient = new (kaspa.RpcClient as any)(rpcConfig);

        if (rpcClient && typeof rpcClient.connect !== 'function') {
            throw new Error('Critical: rpcClient.connect is not a function');
        }

        console.log('[kaspa] Connecting RPC client...');
        await rpcClient.connect({});
        console.log(`[kaspa] RPC connected: ${rpcClient.url ?? 'resolver'}`);

        return rpcClient;
    } catch (error) {
        console.error('[kaspa] RPC initialization failed:', error);
        rpcClient = null;
        throw error;
    }
}

export async function disconnectRpc(): Promise<void> {
    if (rpcClient && typeof rpcClient.disconnect === 'function') {
        console.log('[kaspa] Disconnecting RPC client...');
        await rpcClient.disconnect();
        rpcClient = null;
    }
}
