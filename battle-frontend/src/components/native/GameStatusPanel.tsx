import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import type { NativeGameSnapshot } from '../../domain/nativeGame';
import type { SocketState } from '../../hooks/useGameSocket';
import { Icon } from '../Icon';

interface Props {
    snapshot: NativeGameSnapshot;
    mySlot: 1 | 2 | null;
    myTurn: boolean;
    secondsLeft: number | null;
    connection: SocketState;
    isSubmitting: boolean;
    errorCode: string | null;
    onResign: () => void;
}

const KNOWN_ERRORS = [
    'not_your_turn', 'version_conflict', 'column_full', 'invalid_column',
    'game_over', 'wrong_status', 'not_participant',
];

export function GameStatusPanel({ snapshot, mySlot, myTurn, secondsLeft, connection, isSubmitting, errorCode, onResign }: Props) {
    const { t } = useTranslation();
    const [confirmResign, setConfirmResign] = useState(false);
    const isPlayer = mySlot !== null;
    const winner = snapshot.players.find((p) => p.userId === snapshot.winnerUserId) ?? null;

    let headline: string;
    let tone = 'text-kaspa-primary';
    if (snapshot.status === 'READY') {
        headline = t('native.status.starting');
    } else if (snapshot.status === 'FINISHED') {
        if (snapshot.result === 'DRAW') {
            headline = t('native.status.draw');
            tone = 'text-yellow-400';
        } else if (isPlayer) {
            const iWon = winner?.slot === mySlot;
            headline = iWon ? t('native.status.you_won') : t('native.status.you_lost');
            tone = iWon ? 'text-emerald-400' : 'text-red-400';
        } else {
            headline = t('native.status.won_by', { name: winner?.displayName ?? '?' });
        }
    } else if (isPlayer) {
        headline = myTurn ? t('native.status.your_turn') : t('native.status.opponent_turn');
        tone = myTurn ? 'text-kaspa-primary' : 'text-gray-400';
    } else {
        const current = snapshot.players.find((p) => p.userId === snapshot.currentPlayerId);
        headline = t('native.status.turn_of', { name: current?.displayName ?? '?' });
        tone = 'text-gray-300';
    }

    const reasonKey = snapshot.endReason ? `native.reason.${snapshot.endReason}` : null;

    return (
        <div className="space-y-3" data-testid="game-status-panel">
            <div className="flex items-center justify-between gap-3">
                <h3 role="status" aria-live="polite" className={`text-lg font-black uppercase tracking-tight ${tone}`}>
                    {headline}
                </h3>
                {secondsLeft !== null && (
                    <span
                        aria-label={t('native.status.time_left', { seconds: secondsLeft })}
                        className={`flex items-center gap-1 rounded-lg border px-2 py-1 text-xs font-mono font-bold ${secondsLeft <= 15 ? 'border-red-500/50 text-red-300' : 'border-kaspa-border text-gray-400'}`}
                    >
                        <Icon name="clock" className="h-3.5 w-3.5" />
                        {secondsLeft}s
                    </span>
                )}
            </div>

            {reasonKey && snapshot.status === 'FINISHED' && (
                <p className="text-xs text-gray-400">{t(reasonKey)}</p>
            )}
            {snapshot.status === 'FINISHED' && snapshot.result === 'DRAW' && (
                <p className="rounded-lg border border-yellow-500/30 bg-yellow-900/10 p-3 text-xs font-bold text-yellow-400">
                    {t('native.status.draw_refund')}
                </p>
            )}

            {errorCode && (
                <p role="alert" className="rounded-lg border border-red-500/30 bg-red-900/20 p-2 text-xs font-bold text-red-300">
                    {KNOWN_ERRORS.includes(errorCode) ? t(`native.error.${errorCode}`) : t('native.error.generic')}
                </p>
            )}

            <div className="flex items-center justify-between text-2xs font-bold uppercase tracking-widest text-gray-500">
                <span className="flex items-center gap-1.5">
                    <span className={`inline-block h-2 w-2 rounded-full ${connection === 'live' ? 'bg-emerald-500' : 'animate-pulse bg-yellow-500'}`} />
                    {connection === 'live' ? t('native.connection.live') : t('native.connection.reconnecting')}
                </span>
                <span>{t('native.status.moves', { count: snapshot.moveCount })}</span>
            </div>

            {isPlayer && snapshot.status === 'ACTIVE' && (
                confirmResign ? (
                    <div className="flex gap-2">
                        <button type="button" disabled={isSubmitting} onClick={() => { setConfirmResign(false); onResign(); }} className="btn-danger flex-1 h-10 text-xs font-black uppercase tracking-widest">
                            {t('native.resign.confirm')}
                        </button>
                        <button type="button" onClick={() => setConfirmResign(false)} className="flex-1 h-10 rounded-lg border border-kaspa-border text-xs font-bold uppercase tracking-widest text-gray-400 hover:text-white">
                            {t('native.resign.cancel')}
                        </button>
                    </div>
                ) : (
                    <button type="button" disabled={isSubmitting} onClick={() => setConfirmResign(true)} className="w-full h-10 rounded-lg border border-kaspa-border text-xs font-bold uppercase tracking-widest text-gray-400 transition-colors hover:border-red-500/40 hover:text-red-300">
                        {t('native.resign.button')}
                    </button>
                )
            )}
        </div>
    );
}
