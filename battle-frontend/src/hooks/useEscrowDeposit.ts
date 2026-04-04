import { useCallback } from 'react';
import { useWalletStore } from '../stores/useWalletStore';
import { useMatchStore } from '../stores/useMatchStore';
import { useAuthStore } from '../stores/useAuthStore';
import { sendDeposit, getBalance } from '../kaspa/wallet';
import { submitDeposit } from '../api/matches';
import { getMatchStakeSompi } from '../api/types';
import { getErrorMessage } from '../utils/errors';

export function useEscrowDeposit() {
    const { account, address, setBalance } = useWalletStore();
    const { currentMatch, setDepositing, setLocalDeposit, setError } = useMatchStore();

    const executeDeposit = useCallback(async (playerRole: 'A' | 'B') => {
        if (!currentMatch) {
            setError('Kein Match ausgewaehlt');
            return;
        }

        if (!account) {
            setError('Wallet nicht verbunden oder KasWare wird im WASM-Flow derzeit nicht unterstuetzt.');
            return;
        }

        setDepositing(true);
        try {
            const wagerSompi = getMatchStakeSompi(currentMatch);

            console.log('[useEscrowDeposit] Starte Deposit:', {
                escrowAddress: currentMatch.escrow_address,
                wagerSompi,
                rawStakeKas: currentMatch.stake_kas,
                matchId: currentMatch.id,
            });

            if (!wagerSompi || wagerSompi <= 0) {
                throw new Error('Kein gueltiger Einsatz-Betrag vorhanden.');
            }

            if (!currentMatch.escrow_address) {
                throw new Error('Escrow-Adresse noch nicht generiert. Bitte die Seite neu laden oder kurz warten.');
            }

            const txHash = await sendDeposit(
                account.mnemonicPhrase as string,
                currentMatch.escrow_address,
                wagerSompi,
            );

            console.log('[useEscrowDeposit] TX erfolgreich gesendet:', txHash);
            setLocalDeposit({
                txHash,
                matchId: currentMatch.id,
                userId: useAuthStore.getState().user?.id ?? null,
                walletAddress: address ?? null,
            });

            getBalance(account.xpub).then(setBalance).catch(() => {
                console.warn('[useEscrowDeposit] Balance-Refresh nach Deposit fehlgeschlagen');
            });

            submitDeposit({
                match_id: currentMatch.id,
                tx_hash: txHash,
                player_role: playerRole,
            }).then(() => {
                console.log('[useEscrowDeposit] Backend benachrichtigt');
            }).catch((backendErr) => {
                console.warn('[useEscrowDeposit] Backend-Benachrichtigung fehlgeschlagen (TX war erfolgreich):', backendErr);
            });
        } catch (err) {
            console.error('[useEscrowDeposit] Deposit Error:', err);
            const errorMsg = getErrorMessage(err, 'Unbekannter Fehler');
            setError(`Deposit fehlgeschlagen: ${errorMsg}`);
        }
    }, [account, address, currentMatch, setBalance, setDepositing, setError, setLocalDeposit]);

    return {
        executeDeposit,
        isDepositing: useMatchStore((s) => s.isDepositing),
        depositTxHash: useMatchStore((s) => s.localDeposit?.txHash ?? null),
    };
}
