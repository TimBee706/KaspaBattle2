import { MIN_WAGER_KAS, MAX_WAGER_KAS } from '../config/constants';
import i18n from '../i18n';

export function validateWagerAmount(kasAmount: number): { valid: boolean; error?: string } {
    if (isNaN(kasAmount) || kasAmount <= 0) {
        return { valid: false, error: i18n.t('validation.wager_min_0') };
    }
    if (kasAmount < MIN_WAGER_KAS) {
        return { valid: false, error: i18n.t('validation.wager_min', { amount: MIN_WAGER_KAS }) };
    }
    if (kasAmount > MAX_WAGER_KAS) {
        return { valid: false, error: i18n.t('validation.wager_max', { amount: MAX_WAGER_KAS }) };
    }
    if (!Number.isFinite(kasAmount)) {
        return { valid: false, error: i18n.t('validation.invalid_amount') };
    }
    return { valid: true };
}

export function validateKaspaAddress(address: string): boolean {
    return /^(kaspa|kaspatest):[a-z0-9]{42,63}$/.test(address);
}
