import { RpcClient } from 'kaspa-wasm';
import { initKaspaWasm } from './init';
import { KASPA_NODE_URL } from '../config/constants';

let rpcClient: RpcClient | null = null;

export async function getRpcClient(): Promise<RpcClient> {
    await initKaspaWasm();
    if (rpcClient) return rpcClient;

    rpcClient = new RpcClient({
        url: KASPA_NODE_URL,      // z.B. 'wss://mainnet.kaspa.aspectron.org'
        encoding: 'borsh',        // Borsh ist 40% effizienter als JSON
    });

    await rpcClient.connect();
    return rpcClient;
}

export async function disconnectRpc(): Promise<void> {
    if (rpcClient) {
        await rpcClient.disconnect();
        rpcClient = null;
    }
}
