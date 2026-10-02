import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { act, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import i18n from '../../i18n';
import { NativeGameContainer } from '../../components/native/NativeGameContainer';
import { useAuthStore } from '../../stores/useAuthStore';
import { ALICE, BOB, MATCH_ID, emptyBoard, makeMatch, makeSnapshot } from '../helpers/nativeFixtures';
import type { NativeGameSnapshot } from '../../domain/nativeGame';

vi.mock('../../api/nativeGames', () => ({
    getGame: vi.fn(),
    startGame: vi.fn(),
    postConnectFourMove: vi.fn(),
    resignGame: vi.fn(),
    getFeatures: vi.fn(),
}));
vi.mock('../../api/matches', () => ({ getMatch: vi.fn().mockResolvedValue({}) }));

import * as api from '../../api/nativeGames';

class FakeWebSocket {
    static instances: FakeWebSocket[] = [];
    onopen: (() => void) | null = null;
    onmessage: ((e: { data: string }) => void) | null = null;
    onclose: (() => void) | null = null;
    onerror: (() => void) | null = null;
    closed = false;
    url: string;
    constructor(url: string) {
        this.url = url;
        FakeWebSocket.instances.push(this);
    }
    close() {
        this.closed = true;
        this.onclose?.();
    }
    open() { this.onopen?.(); }
    emit(payload: unknown) { this.onmessage?.({ data: JSON.stringify(payload) }); }
}

function setUser(id: string | null) {
    useAuthStore.setState({
        user: id ? ({ id, display_name: id === ALICE ? 'Alice' : 'Bob' } as never) : null,
        isAuthenticated: !!id,
    });
}

const getGame = vi.mocked(api.getGame);
const postMove = vi.mocked(api.postConnectFourMove);
const startGame = vi.mocked(api.startGame);

describe('NativeGameContainer (server-authoritative game UI)', () => {
    beforeEach(async () => {
        await i18n.changeLanguage('en');
        FakeWebSocket.instances = [];
        vi.stubGlobal('WebSocket', FakeWebSocket);
        vi.clearAllMocks();
        setUser(ALICE);
    });
    afterEach(() => {
        vi.unstubAllGlobals();
    });

    it('loads the full snapshot via GET first and renders the board with both players', async () => {
        const board = emptyBoard();
        board[0][3] = 1;
        board[0][4] = 2;
        getGame.mockResolvedValue(makeSnapshot({ board, moveCount: 2, version: 5, currentPlayerId: ALICE }));
        render(<NativeGameContainer match={makeMatch()} />);

        expect(await screen.findByTestId('native-game')).toBeInTheDocument();
        expect(getGame).toHaveBeenCalledWith(MATCH_ID);
        expect(screen.getByText('Alice')).toBeInTheDocument();
        expect(screen.getByText('Bob')).toBeInTheDocument();
        expect(screen.getByText('Your turn')).toBeInTheDocument();
        expect(screen.getByTestId('player-panel-1')).toHaveAttribute('aria-current', 'true');
    });

    it('sends only the column with expectedVersion and a fresh UUID nonce, then shows the server result', async () => {
        getGame.mockResolvedValue(makeSnapshot({ version: 3 }));
        const after = emptyBoard();
        after[0][2] = 1;
        postMove.mockResolvedValue(makeSnapshot({ version: 4, board: after, moveCount: 1, currentPlayerId: BOB, stateHash: 'h1' }));
        render(<NativeGameContainer match={makeMatch()} />);
        await screen.findByTestId('native-game');

        await userEvent.click(screen.getByRole('button', { name: /Column 3/ }));

        await waitFor(() => expect(postMove).toHaveBeenCalledTimes(1));
        const [matchId, body] = postMove.mock.calls[0];
        expect(matchId).toBe(MATCH_ID);
        expect(Object.keys(body).sort()).toEqual(['clientNonce', 'column', 'expectedVersion']);
        expect(body.column).toBe(2);
        expect(body.expectedVersion).toBe(3);
        expect(body.clientNonce).toMatch(/^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/);
        expect(await screen.findByText("Opponent's turn")).toBeInTheDocument();
    });

    it('cannot move when it is the opponent\'s turn (no request is sent)', async () => {
        getGame.mockResolvedValue(makeSnapshot({ currentPlayerId: BOB }));
        render(<NativeGameContainer match={makeMatch()} />);
        await screen.findByTestId('native-game');
        await userEvent.click(screen.getByRole('button', { name: /Column 1/ }));
        expect(postMove).not.toHaveBeenCalled();
        expect(screen.getByText("Opponent's turn")).toBeInTheDocument();
    });

    it('applies typed WebSocket events only when they are newer, and re-fetches the snapshot on (re)connect', async () => {
        getGame.mockResolvedValue(makeSnapshot({ version: 3, currentPlayerId: BOB }));
        render(<NativeGameContainer match={makeMatch()} />);
        await screen.findByTestId('native-game');
        const ws = FakeWebSocket.instances[0];

        const callsBefore = getGame.mock.calls.length;
        await act(async () => { ws.open(); });
        expect(getGame.mock.calls.length).toBeGreaterThan(callsBefore); // snapshot first on connect

        const moved = emptyBoard();
        moved[0][4] = 2;
        const v4: NativeGameSnapshot = makeSnapshot({ version: 4, board: moved, moveCount: 1, currentPlayerId: ALICE, lastMove: { column: 4, row: 0, playerId: BOB } });
        await act(async () => {
            ws.emit({ type: 'native_game_move', match_id: MATCH_ID, version: 4, move: { sequence: 1, column: 4, row: 0, playerId: BOB, stateHash: 'x' }, state: v4 });
        });
        expect(await screen.findByText('Your turn')).toBeInTheDocument();

        // a stale (older) event must not roll the board back
        await act(async () => {
            ws.emit({ type: 'native_game_state', match_id: MATCH_ID, version: 3, state: makeSnapshot({ version: 3, currentPlayerId: BOB }) });
        });
        expect(screen.getByText('Your turn')).toBeInTheDocument();

        // events for other matches are ignored
        await act(async () => {
            ws.emit({ type: 'native_game_state', match_id: 'other', version: 99, state: makeSnapshot({ matchId: 'other', version: 99, currentPlayerId: BOB }) });
        });
        expect(screen.getByText('Your turn')).toBeInTheDocument();
    });

    it('after a dropped connection the page reconnects and restores the board from GET /game', async () => {
        vi.useFakeTimers({ shouldAdvanceTime: true });
        try {
            getGame.mockResolvedValue(makeSnapshot({ version: 3, currentPlayerId: BOB }));
            render(<NativeGameContainer match={makeMatch()} />);
            await screen.findByTestId('native-game');
            const first = FakeWebSocket.instances[0];
            await act(async () => { first.open(); });

            // Server moved on while we were offline.
            const board = emptyBoard();
            board[0][0] = 1; board[0][1] = 2;
            getGame.mockResolvedValue(makeSnapshot({ version: 6, board, moveCount: 2, currentPlayerId: ALICE }));
            await act(async () => { first.onclose?.(); });
            expect(screen.getByText('Reconnecting…')).toBeInTheDocument();

            await act(async () => { await vi.advanceTimersByTimeAsync(1_100); });
            expect(FakeWebSocket.instances.length).toBe(2);
            await act(async () => { FakeWebSocket.instances[1].open(); });

            expect(await screen.findByText('Your turn')).toBeInTheDocument();
            expect(screen.getByText('2 moves')).toBeInTheDocument();
            expect(screen.getByText('Live')).toBeInTheDocument();
        } finally {
            vi.useRealTimers();
        }
    });

    it('resyncs on a version conflict instead of guessing', async () => {
        getGame.mockResolvedValueOnce(makeSnapshot({ version: 3 }));
        render(<NativeGameContainer match={makeMatch()} />);
        await screen.findByTestId('native-game');

        getGame.mockResolvedValue(makeSnapshot({ version: 5, currentPlayerId: BOB }));
        postMove.mockRejectedValue({ response: { status: 409, data: { error: 'version_conflict' } } });
        await userEvent.click(screen.getByRole('button', { name: /Column 1/ }));

        expect(await screen.findByText("Opponent's turn")).toBeInTheDocument();
        expect(await screen.findByRole('alert')).toHaveTextContent('The game changed');
    });

    it('starts a READY game automatically (idempotent server call) for participants', async () => {
        const ready = makeSnapshot({ status: 'READY', matchStatus: 'READY_TO_PLAY', version: 0, currentPlayerId: ALICE, turnDeadline: null });
        getGame.mockResolvedValue(ready);
        startGame.mockResolvedValue(makeSnapshot({ version: 1 }));
        render(<NativeGameContainer match={makeMatch({ status: 'READY_TO_PLAY' })} />);
        await waitFor(() => expect(startGame).toHaveBeenCalledWith(MATCH_ID));
        expect(await screen.findByText('Your turn')).toBeInTheDocument();
    });

    it('spectators (and logged-out visitors) see the board but cannot play or resign', async () => {
        setUser(null);
        getGame.mockResolvedValue(makeSnapshot({ you: null }));
        render(<NativeGameContainer match={makeMatch()} />);
        await screen.findByTestId('native-game');
        expect(screen.getByText('Alice to move')).toBeInTheDocument();
        await userEvent.click(screen.getByRole('button', { name: /Column 1/ }));
        expect(postMove).not.toHaveBeenCalled();
        expect(screen.queryByRole('button', { name: 'Resign' })).not.toBeInTheDocument();
    });

    it('shows the result, the reason and the refund note for a draw / a win', async () => {
        getGame.mockResolvedValue(makeSnapshot({
            status: 'FINISHED', matchStatus: 'REFUND_PENDING', result: 'DRAW', endReason: 'DRAW', currentPlayerId: null, turnDeadline: null,
        }));
        const { unmount } = render(<NativeGameContainer match={makeMatch({ status: 'REFUND_PENDING' })} />);
        expect(await screen.findByText('Draw')).toBeInTheDocument();
        expect(screen.getByText('No winner: both stakes are refunded.')).toBeInTheDocument();
        unmount();

        getGame.mockResolvedValue(makeSnapshot({
            status: 'FINISHED', matchStatus: 'FINISHED_GAME', result: 'WIN', endReason: 'RESIGNATION', winnerUserId: ALICE,
            currentPlayerId: null, turnDeadline: null,
        }));
        render(<NativeGameContainer match={makeMatch({ status: 'FINISHED_GAME' })} />);
        expect(await screen.findByText('You won!')).toBeInTheDocument();
        expect(screen.getByText('The opponent resigned.')).toBeInTheDocument();
    });

    it('resign asks for confirmation before calling the server', async () => {
        getGame.mockResolvedValue(makeSnapshot());
        vi.mocked(api.resignGame).mockResolvedValue(makeSnapshot({
            status: 'FINISHED', result: 'WIN', endReason: 'RESIGNATION', winnerUserId: BOB, currentPlayerId: null, version: 4, turnDeadline: null,
        }));
        render(<NativeGameContainer match={makeMatch()} />);
        await screen.findByTestId('native-game');
        await userEvent.click(screen.getByRole('button', { name: 'Resign' }));
        expect(api.resignGame).not.toHaveBeenCalled();
        await userEvent.click(screen.getByRole('button', { name: 'Yes, resign' }));
        await waitFor(() => expect(api.resignGame).toHaveBeenCalledWith(MATCH_ID));
        expect(await screen.findByText('You lost')).toBeInTheDocument();
    });
});
