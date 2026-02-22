import { useCallback } from 'react';
import { useWalletStore } from '../stores/useWalletStore';
import { useMatchStore } from '../stores/useMatchStore';
import { sendDeposit } from '../kaspa/wallet';
import { submitDeposit } from '../api/matches';

export function useEscrowDeposit() {
    const { account } = useWalletStore();
    const { currentMatch, setDepositing, setDepositTxHash, setError } = useMatchStore();

    const executeDeposit = useCallback(async (playerRole: 'A' | 'B') => {
        if (!account || !currentMatch) {
            setError('Wallet nicht verbunden oder kein Match ausgewählt');
            return;
        }

        setDepositing(true);
        try {
            // 1. TX an Escrow-Adresse über WASM SDK senden
            const txHash = await sendDeposit(
                account,
                currentMatch.escrow_address,
                currentMatch.wager_amount_sompi,
            );

            // 2. TX-Hash an Backend melden
            await submitDeposit({
                match_id: currentMatch.id,
                tx_hash: txHash,
                player_role: playerRole,
            });

            setDepositTxHash(txHash);
        } catch (err: any) {
            setError(`Deposit fehlgeschlagen: ${err.message}`);
        }
    }, [account, currentMatch, setDepositing, setDepositTxHash, setError]);

    return {
        executeDeposit,
        isDepositing: useMatchStore((s) => s.isDepositing),
        depositTxHash: useMatchStore((s) => s.depositTxHash),
    };
}
