import { useEffect, useRef, useState } from 'react';
import { WS_BASE_URL } from '../config/constants';

export type SocketState = 'connecting' | 'live' | 'reconnecting';

export function buildWsUrl(): string {
    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
    return WS_BASE_URL.startsWith('ws') ? WS_BASE_URL : `${protocol}//${window.location.host}${WS_BASE_URL}`;
}

interface Options {
    enabled: boolean;
    /** Called for every parsed JSON message. */
    onMessage: (data: unknown) => void;
    /**
     * Called every time the socket (re)opens. Consumers MUST reload a full snapshot here:
     * WebSocket events are never the only source of state.
     */
    onOpen: () => void;
}

/** WebSocket with automatic reconnect (exponential backoff, max 15 s). */
export function useGameSocket({ enabled, onMessage, onOpen }: Options): SocketState {
    const [state, setState] = useState<SocketState>('connecting');
    const onMessageRef = useRef(onMessage);
    const onOpenRef = useRef(onOpen);
    // Keep the latest callbacks without re-opening the socket on every render.
    useEffect(() => {
        onMessageRef.current = onMessage;
        onOpenRef.current = onOpen;
    });

    useEffect(() => {
        if (!enabled) return;
        let ws: WebSocket | null = null;
        let closedByUs = false;
        let attempt = 0;
        let timer: ReturnType<typeof setTimeout> | null = null;

        const connect = () => {
            setState(attempt === 0 ? 'connecting' : 'reconnecting');
            try {
                ws = new WebSocket(buildWsUrl());
            } catch {
                scheduleReconnect();
                return;
            }
            ws.onopen = () => {
                attempt = 0;
                setState('live');
                onOpenRef.current();
            };
            ws.onmessage = (event) => {
                try {
                    onMessageRef.current(JSON.parse(event.data as string));
                } catch {
                    /* ignore malformed frames */
                }
            };
            ws.onclose = () => {
                if (!closedByUs) scheduleReconnect();
            };
            ws.onerror = () => ws?.close();
        };

        const scheduleReconnect = () => {
            setState('reconnecting');
            const delay = Math.min(15_000, 1_000 * 2 ** attempt);
            attempt += 1;
            timer = setTimeout(connect, delay);
        };

        connect();
        return () => {
            closedByUs = true;
            if (timer) clearTimeout(timer);
            ws?.close();
        };
    }, [enabled]);

    return state;
}
