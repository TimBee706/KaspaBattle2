import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { getGame, postConnectFourMove, resignGame, startGame } from '../api/nativeGames';
import { useAuthStore } from '../stores/useAuthStore';
import {
    getSlotOf,
    isColumnFull,
    isMyTurn,
    pickNewerSnapshot,
    NATIVE_EVENT_TYPES,
    type NativeGameEvent,
    type NativeGameSnapshot,
} from '../domain/nativeGame';
import { useGameSocket, type SocketState } from './useGameSocket';
import { getApiErrorCode } from '../utils/errors';

const FALLBACK_POLL_MS = 10_000;

function newNonce(): string {
    if (typeof crypto !== 'undefined' && typeof crypto.randomUUID === 'function') return crypto.randomUUID();
    // RFC 4122 v4 fallback for very old browsers / test environments.
    return 'xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx'.replace(/[xy]/g, (c) => {
        const r = (Math.random() * 16) | 0;
        return (c === 'x' ? r : (r & 0x3) | 0x8).toString(16);
    });
}

export interface UseConnectFourGame {
    snapshot: NativeGameSnapshot | null;
    /** true until the first snapshot request finished */
    isLoading: boolean;
    /** 404 game_not_started: funded but the session does not exist yet */
    notStarted: boolean;
    error: string | null;
    isSubmitting: boolean;
    connection: SocketState;
    /** which seat the current user has (null = spectator / not logged in) */
    mySlot: 1 | 2 | null;
    myTurn: boolean;
    /** seconds until the current player forfeits (null if no running clock) */
    secondsLeft: number | null;
    canDrop: (column: number) => boolean;
    dropDisc: (column: number) => Promise<void>;
    resign: () => Promise<void>;
    reload: () => Promise<void>;
}

/**
 * State + actions for one Connect Four game.
 *
 * The server is the only authority: this hook never computes a row, a turn or a winner. Snapshots
 * come from `GET /game` (on mount and after EVERY WebSocket reconnect) and from typed WebSocket
 * events; an event is applied only if its version is newer than what is shown.
 */
export function useConnectFourGame(
    matchId: string | null,
    options: { onMatchUpdate?: () => void } = {},
): UseConnectFourGame {
    const userId = useAuthStore((s) => s.user?.id) ?? null;
    const [snapshot, setSnapshot] = useState<NativeGameSnapshot | null>(null);
    const [isLoading, setLoading] = useState(true);
    const [notStarted, setNotStarted] = useState(false);
    const [error, setError] = useState<string | null>(null);
    const [isSubmitting, setSubmitting] = useState(false);
    const [now, setNow] = useState(() => Date.now());
    const clockOffset = useRef(0);
    const startRequested = useRef(false);
    const onMatchUpdateRef = useRef(options.onMatchUpdate);
    useEffect(() => {
        onMatchUpdateRef.current = options.onMatchUpdate;
    });

    const apply = useCallback((incoming: NativeGameSnapshot) => {
        clockOffset.current = Date.parse(incoming.serverTime) - Date.now();
        setSnapshot((current) => pickNewerSnapshot(current, incoming));
        setNotStarted(false);
    }, []);

    // Action errors (e.g. "the game changed") are transient hints, not a state.
    useEffect(() => {
        if (!error) return;
        const id = setTimeout(() => setError(null), 4_000);
        return () => clearTimeout(id);
    }, [error]);

    const reload = useCallback(async () => {
        if (!matchId) return;
        try {
            apply(await getGame(matchId));
        } catch (err) {
            if (getApiErrorCode(err) === 'game_not_started') {
                setNotStarted(true);
            } else {
                setError(getApiErrorCode(err) ?? 'load_failed');
            }
        } finally {
            setLoading(false);
        }
    }, [matchId, apply]);

    // Reset when switching matches.
    useEffect(() => {
        setSnapshot(null);
        setLoading(true);
        setNotStarted(false);
        setError(null);
        startRequested.current = false;
    }, [matchId]);

    const handleMessage = useCallback(
        (data: unknown) => {
            const event = data as Partial<NativeGameEvent> | null;
            if (!event || typeof event !== 'object') return;
            if (event.type === 'match_update') {
                if (event.match?.id === matchId) onMatchUpdateRef.current?.();
                return;
            }
            if (
                event.type
                && (NATIVE_EVENT_TYPES as readonly string[]).includes(event.type)
                && 'state' in event
                && event.state
                && event.state.matchId === matchId
            ) {
                apply(event.state);
            }
        },
        [matchId, apply],
    );

    // Snapshot first: on mount, and on every (re)connect.
    const connection = useGameSocket({ enabled: !!matchId, onMessage: handleMessage, onOpen: () => void reload() });

    useEffect(() => {
        void reload();
        const id = setInterval(() => void reload(), FALLBACK_POLL_MS);
        return () => clearInterval(id);
    }, [reload]);

    // Local clock for the turn timer.
    useEffect(() => {
        const id = setInterval(() => setNow(Date.now()), 1_000);
        return () => clearInterval(id);
    }, []);

    const mySlot = useMemo(() => (snapshot ? getSlotOf(snapshot, userId) : null), [snapshot, userId]);
    const myTurn = !!snapshot && isMyTurn(snapshot, userId);

    // "Automatic start": the first participant who sees a READY game starts it (idempotent).
    useEffect(() => {
        if (!matchId || !snapshot || snapshot.status !== 'READY' || !mySlot || startRequested.current) return;
        startRequested.current = true;
        startGame(matchId)
            .then(apply)
            .catch(() => {
                startRequested.current = false; // retry on the next snapshot
            });
    }, [matchId, snapshot, mySlot, apply]);

    const secondsLeft = useMemo(() => {
        if (!snapshot || snapshot.status !== 'ACTIVE' || !snapshot.turnDeadline) return null;
        const deadline = Date.parse(snapshot.turnDeadline);
        return Math.max(0, Math.ceil((deadline - (now + clockOffset.current)) / 1_000));
    }, [snapshot, now]);

    const canDrop = useCallback(
        (column: number) =>
            !!snapshot && myTurn && !isSubmitting && column >= 0 && column < snapshot.columns
            && !isColumnFull(snapshot.board, column),
        [snapshot, myTurn, isSubmitting],
    );

    const dropDisc = useCallback(
        async (column: number) => {
            if (!matchId || !snapshot || !canDrop(column)) return;
            setSubmitting(true);
            setError(null);
            try {
                apply(
                    await postConnectFourMove(matchId, {
                        column,
                        expectedVersion: snapshot.version,
                        clientNonce: newNonce(),
                    }),
                );
            } catch (err) {
                const code = getApiErrorCode(err);
                setError(code ?? 'move_failed');
                // Anything but a plain validation error means our view is stale → resync.
                if (code === 'version_conflict' || code === 'not_your_turn' || code === 'game_over' || code === 'wrong_status') {
                    await reload();
                }
            } finally {
                setSubmitting(false);
            }
        },
        [matchId, snapshot, canDrop, apply, reload],
    );

    const resign = useCallback(async () => {
        if (!matchId) return;
        setSubmitting(true);
        try {
            apply(await resignGame(matchId));
        } catch (err) {
            setError(getApiErrorCode(err) ?? 'resign_failed');
            await reload();
        } finally {
            setSubmitting(false);
        }
    }, [matchId, apply, reload]);

    return {
        snapshot, isLoading, notStarted, error, isSubmitting, connection,
        mySlot, myTurn, secondsLeft, canDrop, dropDisc, resign, reload,
    };
}
