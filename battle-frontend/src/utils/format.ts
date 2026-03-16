import { SOMPI_PER_KAS, KASPA_EXPLORER_URL } from '../config/constants';

export function parseLocalFloat(input: string | number): number {
    if (typeof input === 'number') return input;
    // Handle localized numbers (e.g. 49989,99 -> 49989.99)
    const normalized = input.replace(',', '.').replace(/[^0-9.-]/g, '');
    const num = Number(normalized);
    return isNaN(num) ? 0 : num;
}

export function sompiToKas(sompi: number): number {
    if (!sompi || isNaN(sompi)) return 0;
    return sompi / SOMPI_PER_KAS;
}

export function kasToSompi(kas: number | string): number {
    const validKas = parseLocalFloat(kas);
    return Math.round(validKas * SOMPI_PER_KAS);
}

export function formatKas(sompi: number, minDecimals = 2, maxDecimals = 8): string {
    return sompiToKas(sompi).toLocaleString('de-DE', {
        minimumFractionDigits: minDecimals,
        maximumFractionDigits: maxDecimals,
    });
}

export function shortenAddress(address: string | null | undefined, prefixLen = 10, suffixLen = 6): string {
    if (!address) return '—';
    if (address.length <= prefixLen + suffixLen + 3) return address;
    return `${address.slice(0, prefixLen)}...${address.slice(-suffixLen)}`;
}

export function explorerTxUrl(txHash: string): string {
    return `${KASPA_EXPLORER_URL}/txs/${txHash}`;
}

export function explorerAddressUrl(address: string): string {
    return `${KASPA_EXPLORER_URL}/addresses/${address}`;
}
