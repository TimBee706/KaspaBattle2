import { useEffect, useRef } from 'react';
import { getPaymentStatus } from '../api/matches';
import { useMatchStore } from '../stores/useMatchStore';

const POLL_INTERVAL_MS = 3_000; // 3s — fast enough to feel responsive, light enough for testnet

/**
 * Polls GET /matches/:id/payment-status every 3s while the match is AWAITING_FUNDING.
 * Updates `useMatchStore.paymentStatus` reactively.
 * Auto-stops when `both_paid` is true or matchId is null.
 */
export function usePaymentStatus(matchId: string | null) {
    const { setPaymentStatus, currentMatch } = useMatchStore();
    const intervalRef = useRef<ReturnType<typeof setInterval> | null>(null);

    useEffect(() => {
        if (!matchId) {
            setPaymentStatus(null);
            return;
        }

        // Only poll during deposit phase
        const status = currentMatch?.status;
        const isAwaitingFunding = status === 'AWAITING_FUNDING';

        if (!isAwaitingFunding) {
            // Clear stale polling when match advances past deposit phase
            if (intervalRef.current) clearInterval(intervalRef.current);
            return;
        }

        let isMounted = true;

        const fetchStatus = async () => {
            try {
                const ps = await getPaymentStatus(matchId);
                if (!isMounted) return;
                setPaymentStatus(ps);
                // Stop polling once both players have paid
                if (ps.both_paid && intervalRef.current) {
                    clearInterval(intervalRef.current);
                    intervalRef.current = null;
                }
            } catch (err) {
                // Non-blocking: network errors just skip this cycle
                console.warn('[usePaymentStatus] Poll failed:', err);
            }
        };

        // Initial fetch immediately
        fetchStatus();

        // Start interval
        intervalRef.current = setInterval(fetchStatus, POLL_INTERVAL_MS);

        return () => {
            isMounted = false;
            if (intervalRef.current) {
                clearInterval(intervalRef.current);
                intervalRef.current = null;
            }
        };
        // Re-run when matchId or match status changes
    }, [matchId, currentMatch?.status, setPaymentStatus]);
}
