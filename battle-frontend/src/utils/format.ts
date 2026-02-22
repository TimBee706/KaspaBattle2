import { SOMPI_PER_KAS, KASPA_EXPLORER_URL } from '../config/constants';

export function sompiToKas(sompi: number): number {
    return sompi / SOMPI_PER_KAS;
}

export function kasToSompi(kas: number): number {
    return Math.floor(kas * SOMPI_PER_KAS);
}

export function formatKas(sompi: number, decimals = 2): string {
    return sompiToKas(sompi).toLocaleString('de-DE', {
        minimumFractionDigits: decimals,
        maximumFractionDigits: decimals,
    });
}

export function shortenAddress(address: string, prefixLen = 10, suffixLen = 6): string {
    if (address.length <= prefixLen + suffixLen + 3) return address;
    return `${address.slice(0, prefixLen)}...${address.slice(-suffixLen)}`;
}

export function explorerTxUrl(txHash: string): string {
    return `${KASPA_EXPLORER_URL}/txs/${txHash}`;
}

export function explorerAddressUrl(address: string): string {
    return `${KASPA_EXPLORER_URL}/addresses/${address}`;
}
