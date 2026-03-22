import { useCallback } from 'react';
import { useWalletStore } from '../stores/useWalletStore';
import { useMatchStore } from '../stores/useMatchStore';
import { useAuthStore } from '../stores/useAuthStore';
import { sendDeposit, getBalance } from '../kaspa/wallet';
import { submitDeposit } from '../api/matches';

export function useEscrowDeposit() {
    const { account, address, setBalance } = useWalletStore();
    const { currentMatch, setDepositing, setLocalDeposit, setError } = useMatchStore();

    const executeDeposit = useCallback(async (playerRole: 'A' | 'B') => {
        if (!currentMatch) {
            setError('Kein Match ausgewählt');
            return;
        }

        if (!account) {
            setError('Wallet nicht verbunden oder KasWare wird im WASM-Flow derzeit nicht unterstützt.');
            return;
        }

        setDepositing(true);
        try {
            // stake_kas from backend is already in sompi (useLobby.ts multiplies by 10^8)
            // wager_amount_sompi may be undefined (not returned by backend)
            // So use stake_kas directly as sompi, do NOT multiply again!
            const wagerSompi = currentMatch.wager_amount_sompi || (currentMatch as any).stake_kas || 0;

            console.log("🛠️ [useEscrowDeposit] Starte Deposit:", {
                escrowAddress: currentMatch.escrow_address,
                wagerSompi,
                rawStakeKas: (currentMatch as any).stake_kas,
                matchId: currentMatch.id,
            });

            if (!wagerSompi || wagerSompi <= 0) {
                throw new Error("Kein gültiger Einsatz-Betrag vorhanden.");
            }

            if (!currentMatch.escrow_address) {
                throw new Error("Escrow-Adresse noch nicht generiert. Bitte die Seite neu laden oder kurz warten.");
            }

            // 2. TX an Escrow-Adresse über WASM SDK senden
            const txHash = await sendDeposit(
                account,
                currentMatch.escrow_address,
                wagerSompi,
            );

            console.log("✅ [useEscrowDeposit] TX erfolgreich gesendet:", txHash);
            setLocalDeposit({
                txHash,
                matchId: currentMatch.id,
                userId: useAuthStore.getState().user?.id ?? null,
                walletAddress: address ?? null,
            });

            // Sofort Balance neu laden (nicht auf 30s Polling warten)
            getBalance(account).then(setBalance).catch(() => {
                console.warn("⚠️ Balance-Refresh nach Deposit fehlgeschlagen");
            });

            // 3. Backend über TX informieren (fire-and-forget, nicht blockierend)
            submitDeposit({
                match_id: currentMatch.id,
                tx_hash: txHash,
                player_role: playerRole,
            }).then(() => {
                console.log("✅ [useEscrowDeposit] Backend benachrichtigt");
            }).catch((backendErr) => {
                console.warn("⚠️ [useEscrowDeposit] Backend-Benachrichtigung fehlgeschlagen (TX war erfolgreich):", backendErr);
            });

        } catch (err: any) {
            console.error('❌ [useEscrowDeposit] Deposit Error:', err);
            const errorMsg = typeof err === 'string' ? err : (err?.message || JSON.stringify(err) || "Unbekannter Fehler");
            setError(`Deposit fehlgeschlagen: ${errorMsg}`);
        }
    }, [account, address, currentMatch, setDepositing, setLocalDeposit, setError, setBalance]);

    return {
        executeDeposit,
        isDepositing: useMatchStore((s) => s.isDepositing),
        depositTxHash: useMatchStore((s) => s.localDeposit?.txHash ?? null),
    };
}
