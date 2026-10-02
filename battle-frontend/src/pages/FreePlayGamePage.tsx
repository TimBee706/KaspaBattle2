import { useState } from 'react';
import { Link, useNavigate, useParams } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { useFreePlayGame } from '../hooks/useFreePlayGame';
import { leaveFreePlayGame, rematchFreePlayGame } from '../api/freePlay';
import { toBoardView, type FpSnapshot } from '../domain/freePlay';
import { ConnectFourBoard } from '../components/native/ConnectFourBoard';
import { PlayerPanel } from '../components/native/PlayerPanel';
import { FormAlert } from '../components/auth/AuthCard';
import { PageHeader } from '../components/common/PageHeader';
import { Icon } from '../components/Icon';
import { useAuthStore } from '../stores/useAuthStore';
import { getApiErrorCode } from '../utils/errors';

const KNOWN_ERRORS = ['not_your_turn', 'version_conflict', 'column_full', 'invalid_column', 'game_over', 'game_not_running', 'rate_limited'];

export function FreePlayGamePage() {
    const { id } = useParams<{ id: string }>();
    const { t } = useTranslation();
    const navigate = useNavigate();
    const isAuthenticated = useAuthStore((s) => s.isAuthenticated);
    const isAuthLoading = useAuthStore((s) => s.isAuthLoading);
    const game = useFreePlayGame(isAuthenticated ? (id ?? null) : null);
    const [confirmLeave, setConfirmLeave] = useState(false);
    const [busy, setBusy] = useState(false);
    const [actionError, setActionError] = useState<string | null>(null);
    const { snapshot } = game;

    if (isAuthLoading) return null;
    if (!isAuthenticated) {
        return (
            <div className="glass-panel mx-auto max-w-md space-y-4 p-8 text-center">
                <p className="text-gray-300">{t('freeplay.login_required')}</p>
                <Link to={`/login?next=/free-play/${id ?? ''}`} className="btn-primary flex h-11 items-center justify-center">{t('auth.login.submit')}</Link>
            </div>
        );
    }
    if (game.notFound) {
        return <p className="py-16 text-center text-gray-400">{t('freeplay.not_found')} <Link to="/free-play" className="font-bold text-kaspa-primary hover:underline">{t('freeplay.back')}</Link></p>;
    }
    if (!snapshot) {
        return <div className="flex min-h-[40vh] items-center justify-center" data-testid="fp-loading"><div className="h-10 w-10 animate-spin rounded-full border-4 border-kaspa-primary border-t-transparent" /></div>;
    }

    const act = async (fn: () => Promise<FpSnapshot>) => {
        setBusy(true);
        setActionError(null);
        try {
            game.apply(await fn());
        } catch (err) {
            setActionError(getApiErrorCode(err) ?? 'generic');
            await game.reload();
        } finally {
            setBusy(false);
        }
    };

    const opponent = snapshot.players.find((p) => p.slot !== game.mySlot);
    const opponentConnected = opponent?.userId ? game.presence[opponent.userId] : undefined;
    const youWon = snapshot.winnerSlot !== null && snapshot.winnerSlot === game.mySlot;
    const iAmPlayer = game.mySlot !== null;
    const winnerName = snapshot.players.find((p) => p.slot === snapshot.winnerSlot)?.displayName;

    let headline: string;
    let tone = 'text-kaspa-primary';
    if (snapshot.status === 'open') headline = t('freeplay.waiting_for_opponent');
    else if (snapshot.status === 'cancelled') { headline = t('freeplay.lobby_closed'); tone = 'text-gray-400'; }
    else if (snapshot.status === 'finished') {
        if (snapshot.result === 'draw') { headline = t('freeplay.result.draw'); tone = 'text-yellow-400'; }
        else if (snapshot.result === 'abandoned') { headline = t('freeplay.result.abandoned'); tone = 'text-gray-400'; }
        else if (iAmPlayer) { headline = youWon ? t('freeplay.result.you_won') : t('freeplay.result.you_lost'); tone = youWon ? 'text-emerald-400' : 'text-red-400'; }
        else headline = t('freeplay.result.won_by', { name: winnerName ?? '?' });
    } else if (iAmPlayer) { headline = game.myTurn ? t('freeplay.your_turn') : t('freeplay.opponent_turn'); tone = game.myTurn ? 'text-kaspa-primary' : 'text-gray-400'; }
    else headline = t('freeplay.turn_of', { name: snapshot.players.find((p) => p.slot === snapshot.currentSlot)?.displayName ?? '?' });

    const botLabel = snapshot.botDifficulty ? t('freeplay.bot_label', { level: t(`freeplay.diff.${snapshot.botDifficulty}`) }) : t('freeplay.bot');
    const players = snapshot.players.map((p) => ({
        userId: p.userId ?? 'bot',
        displayName: p.isBot ? botLabel : p.displayName,
        slot: p.slot,
        color: p.color,
    }));
    const showBoard = snapshot.status !== 'cancelled';
    const iRequestedRematch = !!snapshot.rematchRequestedBy && snapshot.rematchRequestedBy === snapshot.you?.userId;
    const otherRequestedRematch = !!snapshot.rematchRequestedBy && !iRequestedRematch;

    return (
        <div className="mx-auto max-w-3xl">
            <PageHeader icon="bolt" title={t('freeplay.title')} subtitle={t('freeplay.free_badge')} />
            <div className="glass-panel space-y-4 p-4 sm:p-6" data-testid="free-play-game">
                <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
                    {players.map((p) => (
                        <PlayerPanel
                            key={p.slot}
                            player={p}
                            isYou={p.slot === game.mySlot}
                            isToMove={snapshot.status === 'active' && snapshot.currentSlot === p.slot}
                            isWinner={snapshot.winnerSlot === p.slot}
                        />
                    ))}
                </div>

                {showBoard && (
                    <ConnectFourBoard
                        snapshot={toBoardView(snapshot)}
                        myColor={game.mySlot === 1 ? 'blue' : game.mySlot === 2 ? 'red' : null}
                        canPlay={game.myTurn && !game.isSubmitting}
                        onDrop={(c) => void game.dropDisc(c)}
                    />
                )}

                <div className="space-y-3" data-testid="fp-status">
                    <div className="flex items-center justify-between gap-3">
                        <h2 role="status" aria-live="polite" className={`text-lg font-black uppercase tracking-tight ${tone}`}>{headline}</h2>
                        {game.secondsLeft !== null && (
                            <span aria-label={t('freeplay.time_left', { seconds: game.secondsLeft })} className={`flex items-center gap-1 rounded-lg border px-2 py-1 font-mono text-xs font-bold ${game.secondsLeft <= 20 ? 'border-red-500/50 text-red-300' : 'border-kaspa-border text-gray-400'}`}>
                                <Icon name="clock" className="h-3.5 w-3.5" />{game.secondsLeft}s
                            </span>
                        )}
                    </div>

                    {snapshot.status === 'finished' && snapshot.endReason && <p className="text-xs text-gray-400">{t(`freeplay.reason.${snapshot.endReason}`)}</p>}
                    {snapshot.status === 'open' && <p className="text-sm text-gray-400">{t('freeplay.open_hint')}</p>}
                    {(game.error || actionError) && (
                        <FormAlert kind="error">{t(KNOWN_ERRORS.includes(game.error ?? actionError ?? '') ? `freeplay.errors.${game.error ?? actionError}` : 'freeplay.errors.generic')}</FormAlert>
                    )}

                    <div className="flex items-center justify-between text-2xs font-bold uppercase tracking-widest text-gray-500">
                        <span className="flex items-center gap-1.5">
                            <span className={`inline-block h-2 w-2 rounded-full ${game.connection === 'live' ? 'bg-emerald-500' : 'animate-pulse bg-yellow-500'}`} />
                            {game.connection === 'live' ? t('native.connection.live') : t('native.connection.reconnecting')}
                        </span>
                        {snapshot.opponentKind === 'human' && snapshot.status === 'active' && opponentConnected !== undefined && (
                            <span data-testid="fp-presence" className={opponentConnected ? 'text-emerald-400' : 'text-amber-400'}>
                                {opponentConnected ? t('freeplay.opponent_connected') : t('freeplay.opponent_disconnected')}
                            </span>
                        )}
                        <span>{t('freeplay.moves', { count: snapshot.moveCount })}</span>
                    </div>

                    {snapshot.opponentKind === 'human' && snapshot.status === 'active' && (
                        <p className="text-xs text-gray-500">{t('freeplay.disconnect_rule', { seconds: snapshot.turnTimeoutSecs })}</p>
                    )}
                </div>

                <div className="flex flex-col gap-3 sm:flex-row">
                    {snapshot.status === 'finished' && iAmPlayer && (
                        <>
                            {snapshot.rematchGameId ? (
                                <button type="button" className="btn-primary h-11 flex-1" onClick={() => navigate(`/free-play/${snapshot.rematchGameId}`)}>{t('freeplay.rematch_go')}</button>
                            ) : (
                                <button type="button" className="btn-primary h-11 flex-1" disabled={busy || iRequestedRematch} onClick={() => void act(() => rematchFreePlayGame(snapshot.id))}>
                                    {iRequestedRematch ? t('freeplay.rematch_waiting') : otherRequestedRematch ? t('freeplay.rematch_accept') : t('freeplay.rematch')}
                                </button>
                            )}
                        </>
                    )}
                    {(snapshot.status === 'open' || snapshot.status === 'active') && iAmPlayer && (
                        confirmLeave ? (
                            <div className="flex flex-1 gap-2">
                                <button type="button" className="btn-danger h-11 flex-1 text-sm font-black uppercase" disabled={busy} onClick={() => { setConfirmLeave(false); void act(() => leaveFreePlayGame(snapshot.id)); }}>
                                    {snapshot.status === 'open' ? t('freeplay.close_lobby') : t('freeplay.leave_confirm')}
                                </button>
                                <button type="button" className="h-11 flex-1 rounded-lg border border-kaspa-border text-sm font-bold text-gray-400 hover:text-white" onClick={() => setConfirmLeave(false)}>{t('native.resign.cancel')}</button>
                            </div>
                        ) : (
                            <button type="button" className="h-11 flex-1 rounded-lg border border-kaspa-border text-sm font-bold uppercase tracking-widest text-gray-400 hover:border-red-500/40 hover:text-red-300" onClick={() => setConfirmLeave(true)}>
                                {t('freeplay.leave')}
                            </button>
                        )
                    )}
                    <Link to="/free-play" className="flex h-11 flex-1 items-center justify-center rounded-lg border border-kaspa-border text-sm font-bold uppercase tracking-widest text-gray-300 hover:text-white">{t('freeplay.back')}</Link>
                </div>
            </div>
        </div>
    );
}
