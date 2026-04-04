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

export async function getRpcClient(): Promise<kaspa.RpcClient> {
    try {
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

        const RpcClientClass = kaspa.RpcClient as unknown as RpcClientCtor;
        rpcClient = new RpcClientClass(rpcConfig);

        await rpcClient.connect({});
        return rpcClient;
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
