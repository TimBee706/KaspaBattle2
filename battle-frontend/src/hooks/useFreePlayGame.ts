import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { getFreePlayGame, postFreePlayMove } from '../api/freePlay';
import { useAuthStore } from '../stores/useAuthStore';
import { isColumnFull } from '../domain/nativeGame';
import { isMyTurn, myMark, pickNewer, type FpSnapshot } from '../domain/freePlay';
import { useGameSocket, type SocketState } from './useGameSocket';
import { getApiErrorCode } from '../utils/errors';

const FALLBACK_POLL_MS = 8_000;
const FREE_PLAY_EVENTS = ['free_play_game_started', 'free_play_game_state', 'free_play_move', 'free_play_finished'];

function newNonce(): string {
    if (typeof crypto !== 'undefined' && typeof crypto.randomUUID === 'function') return crypto.randomUUID();
    return 'xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx'.replace(/[xy]/g, (c) => {
        const r = (Math.random() * 16) | 0;
        return (c === 'x' ? r : (r & 0x3) | 0x8).toString(16);
    });
}

export interface UseFreePlayGame {
    snapshot: FpSnapshot | null;
    isLoading: boolean;
    notFound: boolean;
    error: string | null;
    isSubmitting: boolean;
    connection: SocketState;
    mySlot: 1 | 2 | null;
    myTurn: boolean;
    secondsLeft: number | null;
    /** userId → currently connected (live presence of the players) */
    presence: Record<string, boolean>;
    canDrop: (column: number) => boolean;
    dropDisc: (column: number) => Promise<void>;
    apply: (snapshot: FpSnapshot) => void;
    reload: () => Promise<void>;
}

/**
 * State + actions for one Free Play game. Server-authoritative: this hook never computes rows,
 * turns or winners. Snapshot first (mount + every reconnect), WebSocket events only when newer.
 */
export function useFreePlayGame(gameId: string | null): UseFreePlayGame {
    const userId = useAuthStore((s) => s.user?.id) ?? null;
    const [snapshot, setSnapshot] = useState<FpSnapshot | null>(null);
    const [isLoading, setLoading] = useState(true);
    const [notFound, setNotFound] = useState(false);
    const [error, setError] = useState<string | null>(null);
    const [isSubmitting, setSubmitting] = useState(false);
    const [now, setNow] = useState(() => Date.now());
    const [presence, setPresence] = useState<Record<string, boolean>>({});
    const clockOffset = useRef(0);

    const apply = useCallback((incoming: FpSnapshot) => {
        clockOffset.current = Date.parse(incoming.serverTime) - Date.now();
        setSnapshot((cur) => pickNewer(cur, incoming));
        if (incoming.presence) setPresence((p) => ({ ...p, ...incoming.presence }));
        setNotFound(false);
    }, []);

    const reload = useCallback(async () => {
        if (!gameId) return;
        try {
            apply(await getFreePlayGame(gameId));
        } catch (err) {
            const code = getApiErrorCode(err);
            if (code === 'game_not_found') setNotFound(true);
            else setError(code ?? 'load_failed');
        } finally {
            setLoading(false);
        }
    }, [gameId, apply]);

    useEffect(() => {
        setSnapshot(null);
        setLoading(true);
        setNotFound(false);
        setError(null);
        setPresence({});
    }, [gameId]);

    useEffect(() => {
        if (!error) return;
        const id = setTimeout(() => setError(null), 4_000);
        return () => clearTimeout(id);
    }, [error]);

    const handleMessage = useCallback(
        (data: unknown) => {
            const ev = data as { type?: string; gameId?: string; state?: FpSnapshot; userId?: string; connected?: boolean } | null;
            if (!ev || ev.gameId !== gameId) return;
            if (ev.type && FREE_PLAY_EVENTS.includes(ev.type) && ev.state) apply(ev.state);
            if (ev.type === 'free_play_presence' && ev.userId) setPresence((p) => ({ ...p, [ev.userId as string]: !!ev.connected }));
        },
        [gameId, apply],
    );

    const connection = useGameSocket({
        enabled: !!gameId,
        onMessage: handleMessage,
        onOpen: (send) => {
            // Re-subscribe after every (re)connect, then load the full snapshot first.
            if (gameId) send({ type: 'subscribe', gameId });
            void reload();
        },
    });

    useEffect(() => {
        void reload();
        const id = setInterval(() => void reload(), FALLBACK_POLL_MS);
        return () => clearInterval(id);
    }, [reload]);

    useEffect(() => {
        const id = setInterval(() => setNow(Date.now()), 1_000);
        return () => clearInterval(id);
    }, []);

    const mySlot = useMemo(() => (snapshot ? myMark(snapshot, userId) : null), [snapshot, userId]);
    const myTurn = !!snapshot && isMyTurn(snapshot, userId);

    const secondsLeft = useMemo(() => {
        // Bot games have no short clock (idle timeout is an hour) – only show human clocks.
        if (!snapshot || snapshot.status !== 'active' || !snapshot.turnDeadline || snapshot.opponentKind === 'bot') return null;
        return Math.max(0, Math.ceil((Date.parse(snapshot.turnDeadline) - (now + clockOffset.current)) / 1_000));
    }, [snapshot, now]);

    const canDrop = useCallback(
        (column: number) =>
            !!snapshot && myTurn && !isSubmitting && column >= 0 && column < snapshot.columns && !isColumnFull(snapshot.board, column),
        [snapshot, myTurn, isSubmitting],
    );

    const dropDisc = useCallback(
        async (column: number) => {
            if (!gameId || !snapshot || !canDrop(column)) return;
            setSubmitting(true);
            setError(null);
            try {
                apply(await postFreePlayMove(gameId, { column, expectedVersion: snapshot.version, clientNonce: newNonce() }));
            } catch (err) {
                const code = getApiErrorCode(err);
                setError(code ?? 'move_failed');
                if (code && !['column_full', 'invalid_column', 'rate_limited'].includes(code)) await reload();
            } finally {
                setSubmitting(false);
            }
        },
        [gameId, snapshot, canDrop, apply, reload],
    );

    return { snapshot, isLoading, notFound, error, isSubmitting, connection, mySlot, myTurn, secondsLeft, presence, canDrop, dropDisc, apply, reload };
}
