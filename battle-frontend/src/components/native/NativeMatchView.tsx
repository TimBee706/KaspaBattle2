import { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { type BattleMatch, getMatchStakeSompi } from '../../api/types';
import { acceptMatch, getMatch, requestRefund } from '../../api/matches';
import { explorerAddressUrl, explorerTxUrl, formatKas } from '../../utils/format';
import { KASPA_NETWORK } from '../../config/constants';
import { getLobbyRole, isAvailableChallenge, needsPlayerDeposit } from '../../domain/lobby';
import { getAccessBlocker } from '../../domain/access';
import { useAccess } from '../../hooks/useAccess';
import { usePayoutClaim } from '../../hooks/usePayoutClaim';
import { useAuthStore } from '../../stores/useAuthStore';
import { useMatchStore } from '../../stores/useMatchStore';
import { MatchStatusBadge } from '../match/MatchStatusBadge';
import { DepositConfirmModal } from '../match/DepositConfirmModal';
import { Icon } from '../Icon';
import { KaspaCoin } from './ConnectFourCell';
import { NativeGameContainer } from './NativeGameContainer';
import { hasGameSession } from '../../domain/nativeGame';

/**
 * Match page for provider NATIVE. Shares the escrow / deposit / refund building blocks with the
 * FACEIT view but has no FACEIT anywhere: no account check, no match-id step, no FACEIT profile.
 */
export function NativeMatchView({ match }: { match: BattleMatch }) {
    const { t } = useTranslation();
    const navigate = useNavigate();
    const user = useAuthStore((s) => s.user);
    const access = useAccess();
    const setMatch = useMatchStore((s) => s.setMatch);
    const [depositOpen, setDepositOpen] = useState(false);
    const [accepting, setAccepting] = useState(false);
    const [acceptError, setAcceptError] = useState(false);
    const [refundBusy, setRefundBusy] = useState(false);
    const [refundMessage, setRefundMessage] = useState<string | null>(null);
    const payout = usePayoutClaim(match.id);

    const userId = user?.id ?? null;
    const role = getLobbyRole(match, userId);
    const isParticipant = role !== 'viewer';
    const wager = getMatchStakeSompi(match);
    const blocker = getAccessBlocker(access, 'NATIVE');
    const canAccept = isAvailableChallenge(match, userId) && access.canPlayNative;
    const needsDeposit = needsPlayerDeposit(match, userId) && access.canPlayNative;
    const iAmWinner = !!userId && match.winner_user_id === userId;

    const handleAccept = async () => {
        setAccepting(true);
        setAcceptError(false);
        try {
            await acceptMatch({ match_id: match.id });
            navigate(`/escrow/${match.id}`);
        } catch {
            setAcceptError(true);
        } finally {
            setAccepting(false);
        }
    };

    const nameA = match.player_a_display_name || t('match.challenger');
    const nameB = match.player_b_display_name || t('match.opponent');

    return (
        <div className="max-w-4xl mx-auto space-y-4 md:space-y-6" data-testid="native-match-view">
            <div className="flex flex-col md:flex-row justify-between items-start md:items-center gap-4 glass-panel rounded-2xl border border-kaspa-primary/20 shadow-glow-primary p-4 md:p-6">
                <div className="flex items-center gap-4">
                    <div className="w-12 h-12 bg-kaspa-primary/10 rounded-xl flex items-center justify-center gap-0.5 border border-kaspa-primary/20">
                        <KaspaCoin color="blue" className="w-5 h-5" />
                        <KaspaCoin color="red" className="w-5 h-5" />
                    </div>
                    <div>
                        <h1 className="text-2xl font-black tracking-tight">{t('native.games.connect_four')}</h1>
                        <p className="text-kaspa-primary text-2xs font-black uppercase tracking-widest">
                            {t('native.badge_browser')}
                        </p>
                    </div>
                </div>
                <div className="flex flex-col items-start md:items-end">
                    <MatchStatusBadge status={match.status} />
                    <span className="text-[10px] text-gray-500 mt-2 font-mono break-all">ID: {match.id}</span>
                </div>
            </div>

            <div className="grid grid-cols-1 md:grid-cols-3 gap-6">
                <div className="md:col-span-2 space-y-6">
                    {hasGameSession(match) ? (
                        <NativeGameContainer match={match} />
                    ) : (
                        <div className="glass-panel p-6 space-y-3">
                            <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
                                <div className="flex items-center gap-3 rounded-xl border border-kaspa-border bg-kaspa-dark/50 p-3">
                                    <KaspaCoin color="blue" className="h-8 w-8" />
                                    <span className="truncate text-sm font-bold text-white">{nameA}</span>
                                </div>
                                <div className="flex items-center gap-3 rounded-xl border border-kaspa-border bg-kaspa-dark/50 p-3">
                                    <KaspaCoin color="red" className="h-8 w-8" />
                                    <span className="truncate text-sm font-bold text-white">
                                        {match.opponent_user_id ? nameB : t('native.open_seat')}
                                    </span>
                                </div>
                            </div>
                            <p className="text-xs text-gray-400">{t('native.rules_hint')}</p>
                        </div>
                    )}

                    <div className="glass-panel p-6 border border-kaspa-primary/20 rounded-2xl shadow-glow-primary space-y-4">
                        <h3 className="text-xs font-black text-gray-500 uppercase tracking-[0.2em]">{t('match.escrow_wallet')}</h3>
                        <div className="flex items-center justify-between bg-kaspa-dark p-4 rounded-xl border border-kaspa-border">
                            <div className="font-mono text-sm overflow-hidden text-ellipsis whitespace-nowrap mr-4">{match.escrow_address}</div>
                            <a href={explorerAddressUrl(match.escrow_address)} target="_blank" rel="noopener noreferrer" className="text-kaspa-primary hover:underline text-xs shrink-0 font-bold">
                                EXPLORER ↗
                            </a>
                        </div>
                        <div className="grid grid-cols-1 sm:grid-cols-2 gap-4 pt-2">
                            <div className="p-4 bg-kaspa-dark rounded-xl border border-kaspa-border">
                                <span className="text-[10px] text-gray-500 font-bold block mb-1">{t('match.stake_per_player')}</span>
                                <span className="text-xl font-black text-white">{formatKas(wager)} KAS</span>
                            </div>
                            <div className="p-4 bg-kaspa-primary/10 rounded-xl border border-kaspa-primary/20">
                                <span className="text-[10px] text-kaspa-primary font-bold block mb-1">{t('match.total_pot')}</span>
                                <span className="text-xl font-black text-kaspa-primary">{formatKas(wager * 2)} KAS</span>
                            </div>
                        </div>
                    </div>
                </div>

                <div className="space-y-6">
                    <div className="glass-panel p-6 border border-kaspa-primary/20 rounded-2xl shadow-glow-primary space-y-4">
                        <h3 className="text-xs font-black text-gray-500 uppercase tracking-[0.2em]">{t('match.status_actions')}</h3>

                        {match.status === 'OPEN' && !match.opponent_user_id && (
                            <div className="p-4 bg-blue-900/10 border border-blue-500/20 rounded-xl">
                                <p className="text-xs text-blue-400 font-medium">{t('match.open_info')}</p>
                                {!isParticipant && blocker && (
                                    <p className="text-xs text-yellow-500 mt-2 font-bold" data-testid="native-access-hint">
                                        {blocker === 'login' ? t('native.access.login') : t('native.access.wallet')}
                                    </p>
                                )}
                                {canAccept && (
                                    <button onClick={handleAccept} disabled={accepting} className="w-full btn-primary h-12 mt-4 flex items-center justify-center">
                                        {accepting ? '...' : t('match.accept_challenge')}
                                    </button>
                                )}
                                {acceptError && <p role="alert" className="mt-2 text-xs font-bold text-red-400">{t('match.accept_error')}</p>}
                            </div>
                        )}

                        {needsDeposit && (
                            <button
                                onClick={() => { setMatch(match); setDepositOpen(true); }}
                                className="w-full btn-primary h-12 shadow-lg shadow-kaspa-primary/20"
                            >
                                {t('match.deposit_stake')}
                            </button>
                        )}

                        {match.status === 'AWAITING_FUNDING' && (
                            <div className="p-4 bg-yellow-900/10 border border-yellow-500/20 rounded-xl">
                                <p className="text-xs text-yellow-500 font-bold mb-1">{t('match.waiting_funding')}</p>
                                <p className="text-[10px] text-gray-500">{t('native.waiting_funding_info')}</p>
                            </div>
                        )}

                        {(match.status === 'FUNDED' || match.status === 'READY_TO_PLAY') && (
                            <div className="p-4 bg-cyan-900/10 border border-cyan-500/20 rounded-xl">
                                <p className="text-xs text-cyan-400 font-bold">{t('native.starting')}</p>
                            </div>
                        )}

                        {match.status === 'IN_GAME' && (
                            <div className="text-center py-4">
                                <Icon name="bolt" className="w-8 h-8 text-kaspa-primary animate-pulse mb-2 mx-auto" />
                                <p className="text-[11px] text-gray-400">{t('native.running_info')}</p>
                            </div>
                        )}

                        {(match.status === 'FINISHED_GAME' || match.status === 'READY_FOR_PAYOUT') && (
                            <div className="space-y-3" data-testid="native-payout">
                                <div className="p-4 bg-emerald-900/10 border border-emerald-500/20 rounded-xl">
                                    <p className="text-xs text-emerald-400 font-bold">
                                        {match.status === 'FINISHED_GAME' ? t('native.payout.preparing') : t('native.payout.ready')}
                                    </p>
                                </div>
                                {match.status === 'READY_FOR_PAYOUT' && iAmWinner && (
                                    <>
                                        {payout.state === 'done' ? (
                                            <p className="text-xs font-bold text-emerald-400">
                                                {t('native.payout.done')}{' '}
                                                {payout.txId && (
                                                    <a href={explorerTxUrl(payout.txId)} target="_blank" rel="noopener noreferrer" className="underline font-mono">
                                                        {payout.txId.slice(0, 12)}…
                                                    </a>
                                                )}
                                            </p>
                                        ) : (
                                            <button
                                                type="button"
                                                onClick={() => void payout.claim()}
                                                disabled={payout.state === 'signing'}
                                                className="w-full btn-primary h-12"
                                            >
                                                {payout.state === 'signing' ? t('native.payout.signing') : t('native.payout.claim')}
                                            </button>
                                        )}
                                        {payout.error && (
                                            <p role="alert" className="text-xs font-bold text-red-400">
                                                {payout.error === 'wallet_locked' ? t('native.payout.wallet_locked') : t('native.payout.failed')}
                                            </p>
                                        )}
                                    </>
                                )}
                                {match.status === 'READY_FOR_PAYOUT' && isParticipant && !iAmWinner && (
                                    <p className="text-xs text-gray-400">{t('native.payout.waiting_winner')}</p>
                                )}
                            </div>
                        )}

                        {(match.status === 'RESOLVED' || match.status === 'PAID_OUT') && (
                            <div className="bg-emerald-900/20 border border-emerald-500/30 rounded-xl p-4 text-center">
                                <Icon name="trophy" className="w-8 h-8 text-emerald-400 mx-auto mb-2" />
                                <h4 className="text-emerald-500 font-bold uppercase tracking-tighter">{t('match.paid_out')}</h4>
                                {match.payout_tx_hash && (
                                    <a href={explorerTxUrl(match.payout_tx_hash)} target="_blank" rel="noopener noreferrer" className="text-[10px] text-emerald-400 underline font-mono">
                                        TX: {match.payout_tx_hash.slice(0, 16)}...
                                    </a>
                                )}
                            </div>
                        )}

                        {match.status === 'REFUNDED' && (
                            <div className="bg-emerald-900/20 border border-emerald-500/30 rounded-xl p-4 text-center">
                                <Icon name="coin" className="w-8 h-8 text-emerald-400 mx-auto mb-2" />
                                <h4 className="text-emerald-500 font-bold uppercase tracking-tighter">{t('match.refunded')}</h4>
                                <p className="text-xs text-gray-300 my-3">{t('match.refund_complete_info')}</p>
                                {match.refund_tx_hash && (
                                    <a href={explorerTxUrl(match.refund_tx_hash)} target="_blank" rel="noopener noreferrer" className="text-[10px] text-emerald-400 underline font-mono">
                                        TX: {match.refund_tx_hash.slice(0, 16)}...
                                    </a>
                                )}
                            </div>
                        )}

                        {(match.status === 'REFUND_PENDING' || match.status === 'CANCELLED' || match.status === 'DISPUTED') && (
                            <div className="space-y-3">
                                <div className="p-4 bg-yellow-900/10 border border-yellow-500/20 rounded-xl">
                                    <p className="text-xs text-yellow-400 font-bold">
                                        {match.status === 'REFUND_PENDING' ? t('native.refund_pending_draw') : t('match.refund_pending_info')}
                                    </p>
                                </div>
                                {refundMessage && <p className="text-xs font-bold text-emerald-300">{refundMessage}</p>}
                                {isParticipant && (!match.refund_status || match.refund_status === 'none' || match.refund_status === 'failed') && (
                                    <button
                                        type="button"
                                        disabled={refundBusy}
                                        onClick={async () => {
                                            setRefundBusy(true);
                                            try {
                                                const r = await requestRefund(match.id);
                                                setRefundMessage(r.message);
                                                setMatch(await getMatch(match.id));
                                            } catch {
                                                setRefundMessage(t('match.refund_request_error'));
                                            } finally {
                                                setRefundBusy(false);
                                            }
                                        }}
                                        className="w-full bg-yellow-600/20 border border-yellow-500/30 text-yellow-400 hover:bg-yellow-600/30 font-bold text-xs uppercase tracking-widest py-3 rounded-xl"
                                    >
                                        {refundBusy ? '...' : t('match.request_refund')}
                                    </button>
                                )}
                            </div>
                        )}
                    </div>

                    <div className="glass-panel p-6 border border-kaspa-primary/20 rounded-2xl shadow-glow-primary">
                        <h3 className="text-xs font-black text-gray-500 uppercase tracking-[0.2em] mb-4">{t('match.info')}</h3>
                        <div className="space-y-3">
                            <div className="flex justify-between text-[11px]">
                                <span className="text-gray-500 font-bold uppercase">{t('match.platform')}</span>
                                <span className="text-white">{t('native.platform')}</span>
                            </div>
                            <div className="flex justify-between text-[11px]">
                                <span className="text-gray-500 font-bold uppercase">{t('match.network')}</span>
                                <span className="text-kaspa-primary uppercase">{KASPA_NETWORK}</span>
                            </div>
                            <div className="flex justify-between text-[11px]">
                                <span className="text-gray-500 font-bold uppercase">{t('native.result_source')}</span>
                                <span className="text-white">{t('native.result_source_value')}</span>
                            </div>
                        </div>
                    </div>
                </div>
            </div>

            <DepositConfirmModal
                isOpen={depositOpen}
                onClose={() => setDepositOpen(false)}
                matchId={match.id}
                amountSompi={wager}
                playerRole={role === 'creator' ? 'A' : 'B'}
            />
        </div>
    );
}
