import { useCallback, useState } from 'react';
import { signMessage } from 'kaspa-wasm';
import apiClient from '../api/client';
import { importWallet } from '../kaspa/wallet';
import { useWalletStore } from '../stores/useWalletStore';
import { getApiErrorCode, getErrorMessage } from '../utils/errors';

export type PayoutClaimState = 'idle' | 'signing' | 'done' | 'error';

/**
 * The winner approves the payout by signing `Approve payout for match <id>` with their wallet key.
 * The key is derived in the browser from the in-memory mnemonic and never leaves it: only the
 * signature and the public key are sent to the backend (same contract as the wallet login).
 */
export function usePayoutClaim(matchId: string) {
    const [state, setState] = useState<PayoutClaimState>('idle');
    const [error, setError] = useState<string | null>(null);
    const [txId, setTxId] = useState<string | null>(null);

    const claim = useCallback(async () => {
        const mnemonic = useWalletStore.getState().account?.mnemonicPhrase;
        if (!mnemonic) {
            setState('error');
            setError('wallet_locked'); // e.g. after a page reload: reconnect the wallet first
            return;
        }
        setState('signing');
        setError(null);
        try {
            const { connection, ephemeralPrivateKeyHex } = await importWallet(mnemonic);
            const signature = await signMessage({
                message: `Approve payout for match ${matchId}`,
                privateKey: ephemeralPrivateKeyHex,
                noAuxRand: true,
            });
            const res = await apiClient.post<{ tx_id?: string }>(`/matches/${matchId}/payout/submit-signature`, {
                signature_hex: signature,
                public_key: connection.account.publicKey,
            });
            setTxId(res.data.tx_id ?? null);
            setState('done');
        } catch (err) {
            setState('error');
            setError(getApiErrorCode(err) ?? getErrorMessage(err, 'payout_failed'));
        }
    }, [matchId]);

    return { state, error, txId, claim };
}
