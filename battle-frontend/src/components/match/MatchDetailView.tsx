import { useEffect, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { type BattleMatch, getMatchMode, getMatchStakeSompi } from '../../api/types';
import { formatKas, explorerAddressUrl, explorerTxUrl } from '../../utils/format';
import { SUPPORTED_GAMES } from '../../config/constants';
import { MatchStatusBadge } from './MatchStatusBadge';
import { DepositConfirmModal } from './DepositConfirmModal';
import { acceptMatch, getMatch, submitFaceitMatchId, requestRefund } from '../../api/matches';
import { useAuthStore } from '../../stores/useAuthStore';
import { KASPA_NETWORK } from '../../config/constants';
import { useTranslation } from 'react-i18next';
import { useMatchStore } from '../../stores/useMatchStore';
import { getLobbyRole, needsPlayerDeposit, isAvailableChallenge } from '../../domain/lobby';
import { validateFaceitMatchId } from '../../utils/validation';
import { MatchPlayersPanel } from './MatchPlayersPanel';
import { getApiErrorCode, getApiErrorMessage } from '../../utils/errors';

export function MatchDetailView({ match }: { match: BattleMatch }) {
    const navigate = useNavigate();
    const { user, testMode, isFaceitConnected } = useAuthStore();
    const { setMatch } = useMatchStore();
    const [isDepositModalOpen, setIsDepositModalOpen] = useState(false);
    const [isAccepting, setIsAccepting] = useState(false);
    const [faceitMatchIdInput, setFaceitMatchIdInput] = useState('');
    const [isSubmittingFaceitId, setIsSubmittingFaceitId] = useState(false);
    const [faceitSubmitError, setFaceitSubmitError] = useState<string | null>(null);
    const [faceitSubmitSuccess, setFaceitSubmitSuccess] = useState<string | null>(null);
    const [isRequestingRefund, setIsRequestingRefund] = useState(false);
    const [refundMessage, setRefundMessage] = useState<string | null>(null);
    const { t } = useTranslation();
    const game = SUPPORTED_GAMES.find(g => g.id === match.game_id);

    const wagerSompi = getMatchStakeSompi(match);
    const matchMode = getMatchMode(match);

    const currentUserId = user?.id ?? null;
    const lobbyRole = getLobbyRole(match, currentUserId);
    const isPlayerA = lobbyRole === 'creator';
    const isPlayerB = lobbyRole === 'opponent';
    const isParticipant = isPlayerA || isPlayerB;
    const ownSubmittedFaceitId = isPlayerA
        ? match.faceit_match_id_player_a ?? null
        : isPlayerB
            ? match.faceit_match_id_player_b ?? null
            : null;
    const opponentSubmittedFaceitId = isPlayerA
        ? match.faceit_match_id_player_b ?? null
        : isPlayerB
            ? match.faceit_match_id_player_a ?? null
            : null;
    const playerAFaceitMatchId = match.faceit_match_id_player_a ?? null;
    const playerBFaceitMatchId = match.faceit_match_id_player_b ?? null;
    const bothPlayersSubmittedFaceitId = !!playerAFaceitMatchId && !!playerBFaceitMatchId;
    const faceitIdsMismatch = bothPlayersSubmittedFaceitId && playerAFaceitMatchId !== playerBFaceitMatchId;
    const faceitMatchIdLocked = match.status !== 'GAME_ID_INPUT';
    const isFaceitSubmitDisabled =
        isSubmittingFaceitId
        || faceitMatchIdLocked
        || !isParticipant
        || !faceitMatchIdInput.trim();

    // Phase 1: Not yet accepted by anyone
    const canAccept = isAvailableChallenge(match, currentUserId) && !!user && (testMode || isFaceitConnected);

    // Phase 2: Accepted, but this user hasn't deposited
    const needsDeposit = needsPlayerDeposit(match, currentUserId);

    useEffect(() => {
        setFaceitMatchIdInput(ownSubmittedFaceitId ?? '');
        setFaceitSubmitError(null);
        setFaceitSubmitSuccess(null);
    }, [match.id, ownSubmittedFaceitId]);

    const handleAccept = async () => {
        setIsAccepting(true);
        try {
            await acceptMatch({ match_id: match.id });
            navigate(`/escrow/${match.id}`);
        } catch {
            alert(t('match.accept_error'));
        } finally {
            setIsAccepting(false);
        }
    };

    const handleFaceitMatchIdSubmit = async () => {
        if (!isParticipant || faceitMatchIdLocked) return;

        const normalizedFaceitMatchId = faceitMatchIdInput.trim();
        const validation = validateFaceitMatchId(normalizedFaceitMatchId);
        if (!validation.valid) {
            setFaceitSubmitSuccess(null);
            setFaceitSubmitError(validation.error ?? t('match.faceit_submit_error_generic'));
            return;
        }

        setIsSubmittingFaceitId(true);
        setFaceitSubmitError(null);
        setFaceitSubmitSuccess(null);

        try {
            const response = await submitFaceitMatchId({
                match_id: match.id,
                faceit_match_id: normalizedFaceitMatchId,
            });
            const refreshedMatch = await getMatch(match.id);
            setMatch(refreshedMatch);
            setFaceitMatchIdInput(normalizedFaceitMatchId);
            setFaceitSubmitSuccess(
                response.status === 'confirmed'
                    ? t('match.faceit_submit_confirmed')
                    : t('match.faceit_submit_waiting'),
            );
        } catch (err) {
            const apiErrorCode = getApiErrorCode(err);
            const apiMessage = getApiErrorMessage(err);

            if (apiErrorCode === 'faceit_id_mismatch') {
                setFaceitSubmitError(t('match.faceit_submit_mismatch'));
            } else if (apiErrorCode === 'invalid_faceit_match_id') {
                setFaceitSubmitError(t('validation.faceit_match_id_invalid'));
            } else if (apiErrorCode === 'wrong_status') {
                setFaceitSubmitError(t('match.faceit_submit_locked'));
            } else {
                setFaceitSubmitError(apiMessage || t('match.faceit_submit_error_generic'));
            }

            try {
                const refreshedMatch = await getMatch(match.id);
                setMatch(refreshedMatch);
            } catch (refreshErr) {
                console.warn('[MatchDetailView] Failed to refresh match after FaceIT submit error', refreshErr);
            }
        } finally {
            setIsSubmittingFaceitId(false);
        }
    };

    return (
        <div className="max-w-4xl mx-auto space-y-6">
            <div className="flex flex-col md:flex-row justify-between items-start md:items-center gap-4 bg-kaspa-card p-6 rounded-2xl border border-kaspa-border relative overflow-hidden">
                <div className="absolute top-0 right-0 p-8 opacity-5">
                    {game?.icon && <img src={game.icon} alt={game.name} className="w-48 h-48 object-contain grayscale" />}
                </div>

                <div className="flex items-center gap-4">
                    <div className="w-12 h-12 bg-kaspa-primary/10 rounded-xl flex items-center justify-center p-2 border border-kaspa-primary/20">
                        {game?.icon && <img src={game.icon} alt={game.name} className="w-full h-full object-contain" />}
                    </div>
                    <div>
                        <h1 className="text-2xl font-black tracking-tight">{game?.name}</h1>
                        <p className="text-gray-400 text-xs font-bold uppercase tracking-widest">{t('match.detail_title', { game: matchMode })}</p>
                    </div>
                </div>

                <div className="flex flex-col items-end">
                    <MatchStatusBadge status={match.status} />
                    <span className="text-[10px] text-gray-500 mt-2 font-mono">ID: {match.id}</span>
                </div>
            </div>

            <div className="grid grid-cols-1 md:grid-cols-3 gap-6">
                <div className="md:col-span-2 space-y-6">
                    {/* Players Panel – echte FACEIT-Profile */}
                    <MatchPlayersPanel match={match} />

                    {/* Escrow Details */}
                    <div className="card space-y-4">
                        <h3 className="text-xs font-black text-gray-500 uppercase tracking-[0.2em]">{t('match.escrow_wallet')}</h3>
                        <div className="flex items-center justify-between bg-kaspa-dark p-4 rounded-xl border border-kaspa-border">
                            <div className="font-mono text-sm overflow-hidden text-ellipsis whitespace-nowrap mr-4">
                                {match.escrow_address}
                            </div>
                            <a
                                href={explorerAddressUrl(match.escrow_address)}
                                target="_blank"
                                className="text-kaspa-primary hover:underline text-xs shrink-0 font-bold"
                            >
                                EXPLORER ↗
                            </a>
                        </div>

                        <div className="grid grid-cols-2 gap-4 pt-2">
                            <div className="p-4 bg-kaspa-dark rounded-xl border border-kaspa-border">
                                <span className="text-[10px] text-gray-500 font-bold block mb-1">{t('match.stake_per_player')}</span>
                                <span className="text-xl font-black text-white">{formatKas(wagerSompi)} KAS</span>
                            </div>
                            <div className="p-4 bg-kaspa-primary/10 rounded-xl border border-kaspa-primary/20">
                                <span className="text-[10px] text-kaspa-primary font-bold block mb-1">{t('match.total_pot')}</span>
                                <span className="text-xl font-black text-kaspa-primary">{formatKas(wagerSompi * 2)} KAS</span>
                            </div>
                        </div>
                    </div>
                </div>

                <div className="space-y-6">
                    <div className="card space-y-4">
                        <h3 className="text-xs font-black text-gray-500 uppercase tracking-[0.2em]">{t('match.status_actions')}</h3>

                        {match.status === 'OPEN' && !match.opponent_user_id && (
                            <div className="p-4 bg-blue-900/10 border border-blue-500/20 rounded-xl">
                                <p className="text-xs text-blue-400 font-medium">{t('match.open_info')}</p>
                                {!isFaceitConnected && !testMode && !isPlayerA && (
                                    <p className="text-xs text-yellow-500 mt-2 font-bold">FaceIT muss verbunden sein, um beizutreten.</p>
                                )}
                                {canAccept && (
                                    <button
                                        onClick={handleAccept}
                                        disabled={isAccepting}
                                        className="w-full btn-primary h-12 mt-4 flex items-center justify-center"
                                    >
                                        {isAccepting ? '...' : t('match.accept_challenge')}
                                    </button>
                                )}
                            </div>
                        )}

                        {needsDeposit && (
                                <button
                                    onClick={() => {
                                        setMatch(match);
                                        setIsDepositModalOpen(true);
                                    }}
                                    className="w-full btn-primary h-12 shadow-lg shadow-kaspa-primary/20 animate-in fade-in zoom-in-95"
                                >
                                    {t('match.deposit_stake')}
                                </button>
                            )}

                        {match.status === 'AWAITING_FUNDING' && (
                            <div className="p-4 bg-yellow-900/10 border border-yellow-500/20 rounded-xl">
                                <p className="text-xs text-yellow-500 font-bold mb-1">{t('match.waiting_funding')}</p>
                                <p className="text-[10px] text-gray-500">{t('match.waiting_funding_info')}</p>
                            </div>
                        )}

                        {match.status === 'LOCKED' && (
                            <div className="text-center py-6">
                                <div className="text-4xl animate-pulse mb-4">🎮</div>
                                <h4 className="font-bold text-kaspa-primary">{t('match.running')}</h4>
                                <p className="text-[10px] text-gray-500 mt-2">{t('match.running_info')}</p>
                            </div>
                        )}

                        {match.status === 'GAME_ID_INPUT' && (
                            <div className="p-4 bg-cyan-900/10 border border-cyan-500/20 rounded-xl">
                                <p className="text-xs text-cyan-400 font-bold mb-3">{t('match.game_id_input')}</p>

                                {isParticipant ? (
                                    <div className="space-y-3">
                                        <div className="grid grid-cols-2 gap-2 text-[10px] font-bold uppercase tracking-wide">
                                            <div className={`rounded-lg border px-3 py-2 ${ownSubmittedFaceitId ? 'border-emerald-500/30 bg-emerald-900/20 text-emerald-300' : 'border-cyan-500/20 bg-kaspa-dark text-gray-400'}`}>
                                                {ownSubmittedFaceitId ? t('match.faceit_status_you_submitted') : t('match.faceit_status_you_pending')}
                                            </div>
                                            <div className={`rounded-lg border px-3 py-2 ${opponentSubmittedFaceitId ? 'border-emerald-500/30 bg-emerald-900/20 text-emerald-300' : 'border-cyan-500/20 bg-kaspa-dark text-gray-400'}`}>
                                                {opponentSubmittedFaceitId ? t('match.faceit_status_opponent_submitted') : t('match.faceit_status_opponent_pending')}
                                            </div>
                                        </div>

                                        {faceitIdsMismatch && (
                                            <div className="p-3 bg-red-900/20 border border-red-500/30 rounded-lg">
                                                <p className="text-xs text-red-300 font-bold">{t('match.faceit_submit_mismatch')}</p>
                                            </div>
                                        )}

                                        {faceitSubmitError && (
                                            <div className="p-3 bg-red-900/20 border border-red-500/30 rounded-lg">
                                                <p className="text-xs text-red-300 font-bold">{faceitSubmitError}</p>
                                            </div>
                                        )}

                                        {faceitSubmitSuccess && (
                                            <div className="p-3 bg-emerald-900/20 border border-emerald-500/30 rounded-lg">
                                                <p className="text-xs text-emerald-300 font-bold">{faceitSubmitSuccess}</p>
                                            </div>
                                        )}

                                        <div>
                                            <input
                                                type="text"
                                                value={faceitMatchIdInput}
                                                onChange={(e) => setFaceitMatchIdInput(e.target.value)}
                                                disabled={isSubmittingFaceitId || faceitMatchIdLocked}
                                                placeholder={t('match.faceit_input_placeholder')}
                                                className="w-full bg-kaspa-dark border border-cyan-500/20 rounded-lg px-3 py-3 text-sm text-white font-mono focus:border-kaspa-primary outline-none transition-colors disabled:opacity-60"
                                            />
                                            <p className="text-[10px] text-gray-400 mt-2">{t('match.game_id_input_info')}</p>
                                        </div>

                                        <button
                                            type="button"
                                            onClick={handleFaceitMatchIdSubmit}
                                            disabled={isFaceitSubmitDisabled}
                                            className="w-full btn-primary h-11 flex items-center justify-center disabled:opacity-50 disabled:grayscale"
                                        >
                                            {isSubmittingFaceitId
                                                ? t('match.faceit_submit_loading')
                                                : ownSubmittedFaceitId
                                                    ? t('match.faceit_submit_update')
                                                    : t('match.faceit_submit')}
                                        </button>
                                    </div>
                                ) : (
                                    <p className="text-[10px] text-gray-400">{t('match.game_id_input_info')}</p>
                                )}
                            </div>
                        )}

                        {match.status === 'PAID_OUT' && (
                            <div className="bg-emerald-900/20 border border-emerald-500/30 rounded-xl p-4 text-center">
                                <div className="text-3xl mb-2">💎</div>
                                <h4 className="text-emerald-500 font-bold uppercase tracking-tighter">{t('match.paid_out')}</h4>
                                <p className="text-xs text-white my-3 font-bold">{t('match.winner_message', { name: match.winner_faceit_nickname })}</p>
                                <a href={explorerTxUrl(match.payout_tx_hash!)} target="_blank" className="text-[10px] text-emerald-400 underline font-mono">
                                    TX: {match.payout_tx_hash?.slice(0, 16)}...
                                </a>
                            </div>
                        )}

                        {/* Refund Status Display */}
                        {match.status === 'REFUNDED' && (
                            <div className="bg-emerald-900/20 border border-emerald-500/30 rounded-xl p-4 text-center">
                                <div className="text-3xl mb-2">💸</div>
                                <h4 className="text-emerald-500 font-bold uppercase tracking-tighter">{t('match.refunded')}</h4>
                                <p className="text-xs text-gray-300 my-3">{t('match.refund_complete_info')}</p>
                                {match.refund_tx_hash && (
                                    <a href={explorerTxUrl(match.refund_tx_hash)} target="_blank" className="text-[10px] text-emerald-400 underline font-mono">
                                        TX: {match.refund_tx_hash.slice(0, 16)}...
                                    </a>
                                )}
                            </div>
                        )}

                        {/* Cancelled/Disputed with pending refund */}
                        {(match.status === 'CANCELLED' || match.status === 'DISPUTED') && (
                            <div className="space-y-3">
                                {(!match.refund_status || match.refund_status === 'none' || match.refund_status === 'pending' || match.refund_status === 'pending_manual') && (
                                    <div className="p-4 bg-yellow-900/10 border border-yellow-500/20 rounded-xl">
                                        <p className="text-xs text-yellow-400 font-bold">{t('match.refund_pending_info')}</p>
                                    </div>
                                )}

                                {match.refund_status === 'failed' && (
                                    <div className="p-4 bg-red-900/10 border border-red-500/20 rounded-xl">
                                        <p className="text-xs text-red-400 font-bold">{t('match.refund_failed_info')}</p>
                                    </div>
                                )}

                                {refundMessage && (
                                    <div className="p-3 bg-emerald-900/20 border border-emerald-500/30 rounded-lg">
                                        <p className="text-xs text-emerald-300 font-bold">{refundMessage}</p>
                                    </div>
                                )}

                                {/* Refund Request Button — visible for participants when refund not yet successful */}
                                {isParticipant && (!match.refund_status || match.refund_status === 'none' || match.refund_status === 'failed') && (
                                    <button
                                        onClick={async () => {
                                            setIsRequestingRefund(true);
                                            setRefundMessage(null);
                                            try {
                                                const response = await requestRefund(match.id);
                                                setRefundMessage(response.message);
                                                const refreshed = await getMatch(match.id);
                                                setMatch(refreshed);
                                            } catch {
                                                setRefundMessage(t('match.refund_request_error'));
                                            } finally {
                                                setIsRequestingRefund(false);
                                            }
                                        }}
                                        disabled={isRequestingRefund}
                                        className="w-full bg-yellow-600/20 border border-yellow-500/30 text-yellow-400 hover:bg-yellow-600/30 font-bold text-xs uppercase tracking-widest py-3 rounded-xl transition-colors disabled:opacity-50"
                                    >
                                        {isRequestingRefund ? '...' : t('match.request_refund')}
                                    </button>
                                )}
                            </div>
                        )}
                    </div>

                    <div className="card">
                        <h3 className="text-xs font-black text-gray-500 uppercase tracking-[0.2em] mb-4">{t('match.info')}</h3>
                        <div className="space-y-3">
                            <div className="flex justify-between text-[11px]">
                                <span className="text-gray-500 font-bold uppercase">{t('match.platform')}</span>
                                <span className="text-white">FACEIT</span>
                            </div>
                            <div className="flex justify-between text-[11px]">
                                <span className="text-gray-500 font-bold uppercase">{t('match.network')}</span>
                                <span className="text-kaspa-primary uppercase">{KASPA_NETWORK}</span>
                            </div>
                            <div className="flex justify-between text-[11px]">
                                <span className="text-gray-500 font-bold uppercase">{t('match.fee')}</span>
                                <span className="text-white">5.00%</span>
                            </div>
                        </div>
                    </div>
                </div>
            </div>

            <DepositConfirmModal
                isOpen={isDepositModalOpen}
                onClose={() => setIsDepositModalOpen(false)}
                matchId={match.id}
                amountSompi={wagerSompi}
                playerRole={isPlayerA ? 'A' : 'B'}
            />
        </div>
    );
}
