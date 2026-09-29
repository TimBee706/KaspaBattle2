export class KaspaRpcError extends Error {
    code: string;
    technicalDetail?: string;

    constructor(code: string, message: string, options?: { technicalDetail?: string }) {
        super(message);
        this.name = 'KaspaRpcError';
        this.code = code;
        this.technicalDetail = options?.technicalDetail;
    }
}

export type KaspaRpcErrorCode =
    | 'KASPA_NODE_UNREACHABLE'
    | 'KASPA_TLS_ERROR'
    | 'KASPA_TIMEOUT'
    | 'KASPA_WRONG_NETWORK'
    | 'KASPA_NODE_NOT_SYNCED'
    | 'KASPA_NO_UTXO_INDEX'
    | 'KASPA_ENCODING_ERROR'
    | 'KASPA_RPC_UNAVAILABLE';

/** Maps a raw connect/RPC failure onto a user-meaningful category. */
export function classifyRpcFailure(e: unknown): KaspaRpcErrorCode {
    if (e instanceof KaspaRpcError && e.code.startsWith('KASPA_')) return e.code as KaspaRpcErrorCode;
    const m = String(e instanceof Error ? e.message : e).toLowerCase();
    if (/timeout|timed out/.test(m)) return 'KASPA_TIMEOUT';
    if (/certificate|tls|ssl|handshake|mixed content|insecure/.test(m)) return 'KASPA_TLS_ERROR';
    if (/borsh|decode|deserializ|encoding|protocol/.test(m)) return 'KASPA_ENCODING_ERROR';
    return 'KASPA_NODE_UNREACHABLE';
}

const CODE_MESSAGES: Record<KaspaRpcErrorCode, (net: string) => string> = {
    KASPA_NODE_UNREACHABLE: () => 'Der konfigurierte Kaspa-Node ist nicht erreichbar.',
    KASPA_TLS_ERROR: () => 'TLS/WSS-Verbindung zum Kaspa-Node fehlgeschlagen (Zertifikat oder Proxy-Konfiguration).',
    KASPA_TIMEOUT: () => 'Zeitüberschreitung bei der Verbindung zum Kaspa-Node.',
    KASPA_WRONG_NETWORK: (net) => `Der Kaspa-Node läuft nicht im erwarteten Netzwerk (${net}).`,
    KASPA_NODE_NOT_SYNCED: () => 'Der Kaspa-Node ist noch nicht synchronisiert.',
    KASPA_NO_UTXO_INDEX: () => 'Auf dem Kaspa-Node ist der UTXO-Index nicht aktiv.',
    KASPA_ENCODING_ERROR: () => 'RPC-Protokoll/Encoding passt nicht (Borsh-wRPC erwartet).',
    KASPA_RPC_UNAVAILABLE: (net) => `Keine ${net} Node erreichbar.`,
};

export function rpcErrorMessage(code: KaspaRpcErrorCode, network: string): string {
    return CODE_MESSAGES[code](network);
}
