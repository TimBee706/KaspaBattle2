import { useEffect, useRef } from 'react';
import { useMatchStore } from '../stores/useMatchStore';
import { getMatch } from '../api/matches';
import { MATCH_STATUS_POLL_INTERVAL_MS } from '../config/constants';

// Pollt den Match-Status alle 5 Sekunden, solange der Status nicht final ist
export function useMatchPolling(matchId: string | null) {
    const { setMatch, setError } = useMatchStore();
    const intervalRef = useRef<ReturnType<typeof setInterval> | null>(null);

    useEffect(() => {
        if (!matchId) return;

        const FINAL_STATES = ['PAID_OUT', 'CANCELLED', 'DISPUTED'];

        const poll = async () => {
            try {
                const match = await getMatch(matchId);
                setMatch(match);

                // Polling stoppen bei finalem Status
                if (FINAL_STATES.includes(match.status) && intervalRef.current) {
                    clearInterval(intervalRef.current);
                    intervalRef.current = null;
                }
            } catch (err: any) {
                setError(err.message);
            }
        };

        // Sofort einmal abrufen
        poll();

        // Dann regelmäßig
        intervalRef.current = setInterval(poll, MATCH_STATUS_POLL_INTERVAL_MS);

        return () => {
            if (intervalRef.current) {
                clearInterval(intervalRef.current);
                intervalRef.current = null;
            }
        };
    }, [matchId, setMatch, setError]);
}
