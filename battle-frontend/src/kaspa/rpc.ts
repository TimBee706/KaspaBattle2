import * as kaspa from 'kaspa-wasm';
import { initKaspaWasm } from './init';
import { KASPA_NODE_URL, KASPA_NETWORK } from '../config/constants';

let rpcClient: kaspa.RpcClient | null = null;

export async function getRpcClient(): Promise<kaspa.RpcClient> {
    try {
        console.log("🔄 Initialisiere Kaspa WASM...");
        await initKaspaWasm();

        if (rpcClient) return rpcClient;

        // Nutze positional arguments gemäß Kaspa WASM Docs
        console.log(`🔌 Erstelle RpcClient für Adresse: ${KASPA_NODE_URL}`);
        rpcClient = new (kaspa.RpcClient as any)({
            url: KASPA_NODE_URL,
            encoding: kaspa.Encoding.Borsh,
            networkId: KASPA_NETWORK
        });

        if (rpcClient && typeof rpcClient.connect !== 'function') {
            throw new Error("KRITISCH: rpcClient.connect ist keine Funktion! Überprüfe die WASM Instanziierung.");
        }

        console.log("⏳ Verbinde mit Kaspa Node...");
        await rpcClient!.connect({});
        console.log("✅ RpcClient erfolgreich verbunden!");

        return rpcClient!;
    } catch (error) {
        console.error("❌ Fehler bei der RPC-Initialisierung:", error);
        rpcClient = null;
        throw error;
    }
}

export async function disconnectRpc(): Promise<void> {
    if (rpcClient && typeof rpcClient.disconnect === 'function') {
        console.log("🔌 Trenne RPC-Verbindung...");
        await rpcClient.disconnect();
        rpcClient = null;
    }
}
