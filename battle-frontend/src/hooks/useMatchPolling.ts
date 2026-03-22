import { useEffect, useRef } from 'react';
import { useMatchStore } from '../stores/useMatchStore';
import { getMatch } from '../api/matches';
import { MATCH_STATUS_POLL_INTERVAL_MS, WS_BASE_URL } from '../config/constants';
import type { BattleMatch } from '../api/types';

// Connects to WebSocket for real-time updates and falls back to polling periodically
export function useMatchPolling(matchId: string | null) {
    const { setMatch, setError } = useMatchStore();
    const intervalRef = useRef<ReturnType<typeof setInterval> | null>(null);
    const wsRef = useRef<WebSocket | null>(null);

    useEffect(() => {
        if (!matchId) return;

        const FINAL_STATES = ['PAID_OUT', 'CANCELLED', 'DISPUTED'];

        const handleMatchUpdate = (match: BattleMatch) => {
            setMatch(match);
            if (FINAL_STATES.includes(match.status)) {
                if (intervalRef.current) clearInterval(intervalRef.current);
                if (wsRef.current) wsRef.current.close();
            }
        };

        const poll = async () => {
            try {
                const match = await getMatch(matchId);
                handleMatchUpdate(match);
            } catch (err: any) {
                setError(err.message);
            }
        };

        // 1. Initial HTTP Fetch
        poll();

        // 2. Setup WebSocket
        const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
        const wsUrl = WS_BASE_URL.startsWith('ws')
            ? WS_BASE_URL
            : `${protocol}//${window.location.host}${WS_BASE_URL}`;

        const ws = new WebSocket(wsUrl);
        wsRef.current = ws;

        ws.onmessage = (event) => {
            try {
                const data = JSON.parse(event.data);
                // The backend broadcasts the match payload. We only care about our matchId.
                if (data && data.id === matchId) {
                    console.log('WS Match Update received:', data.status);
                    handleMatchUpdate(data as BattleMatch);
                }
            } catch (e) {
                console.error('WS Parse Error', e);
            }
        };

        ws.onclose = () => {
            console.log('WS Connection closed');
            // If WS disconnects, the polling fallback will keep it somewhat alive
        };

        // 3. Fallback Polling (e.g. if WS fails or connection drops)
        intervalRef.current = setInterval(poll, MATCH_STATUS_POLL_INTERVAL_MS * 2);

        return () => {
            if (intervalRef.current) clearInterval(intervalRef.current);
            if (wsRef.current) wsRef.current.close();
        };
    }, [matchId, setMatch, setError]);
}

