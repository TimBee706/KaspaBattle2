import { MIN_WAGER_KAS, MAX_WAGER_KAS } from '../config/constants';

export function validateWagerAmount(kasAmount: number): { valid: boolean; error?: string } {
    if (isNaN(kasAmount) || kasAmount <= 0) {
        return { valid: false, error: 'Einsatz muss größer als 0 sein' };
    }
    if (kasAmount < MIN_WAGER_KAS) {
        return { valid: false, error: `Mindest-Einsatz: ${MIN_WAGER_KAS} KAS` };
    }
    if (kasAmount > MAX_WAGER_KAS) {
        return { valid: false, error: `Maximal-Einsatz: ${MAX_WAGER_KAS} KAS` };
    }
    if (!Number.isFinite(kasAmount)) {
        return { valid: false, error: 'Ungültiger Betrag' };
    }
    return { valid: true };
}

export function validateKaspaAddress(address: string): boolean {
    return /^kaspa:[a-z0-9]{61,63}$/.test(address);
}
