import { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import type { BattleMatch } from '../../api/types';
import { formatKas, explorerAddressUrl, explorerTxUrl } from '../../utils/format';
import { SUPPORTED_GAMES } from '../../config/constants';
import { MatchStatusBadge } from './MatchStatusBadge';
import { DepositConfirmModal } from './DepositConfirmModal';
import { acceptMatch } from '../../api/matches';
import { useAuthStore } from '../../stores/useAuthStore';
import { KASPA_NETWORK } from '../../config/constants';
import { useTranslation } from 'react-i18next';
import { useMatchStore } from '../../stores/useMatchStore';

export function MatchDetailView({ match }: { match: BattleMatch }) {
    const navigate = useNavigate();
    const { user, testMode, isFaceitConnected } = useAuthStore();
    const { setMatch } = useMatchStore();
    const [isDepositModalOpen, setIsDepositModalOpen] = useState(false);
    const [isAccepting, setIsAccepting] = useState(false);
    const { t } = useTranslation();
    const game = SUPPORTED_GAMES.find(g => g.id === match.game_id);

    const wagerSompi = match.wager_amount_sompi || (match as any).stake_kas || 0;
    const matchMode = match.match_mode || (match as any).mode || 'BO1';

    const isPlayerA = user?.id === match.creator_user_id;
    const isPlayerB = user?.faceit_id === match.player_b_faceit_id || user?.id === match.opponent_user_id;

    // Phase 1: Not yet accepted by anyone
    const canAccept = match.status === 'OPEN' && !match.opponent_user_id && !isPlayerA && !!user && (testMode || isFaceitConnected);

    // Phase 2: Accepted, but this user hasn't deposited
    const needsDepositA = (match.status === 'OPEN' || match.status === 'AWAITING_FUNDING') && isPlayerA && !match.player_a_deposit_tx_hash;
    const needsDepositB = (match.status === 'OPEN' || match.status === 'AWAITING_FUNDING') && isPlayerB && !!match.opponent_user_id && !match.player_b_deposit_tx_hash;
    const needsDeposit = needsDepositA || needsDepositB;

    const handleAccept = async () => {
        setIsAccepting(true);
        try {
            await acceptMatch({ match_id: match.id });
            navigate(`/escrow/${match.id}`);
        } catch (err) {
            alert(t('match.accept_error'));
        } finally {
            setIsAccepting(false);
        }
    };

    return (
        <div className="max-w-4xl mx-auto space-y-6">
            <div className="flex flex-col md:flex-row justify-between items-start md:items-center gap-4 bg-kaspa-card p-6 rounded-2xl border border-kaspa-border relative overflow-hidden">
                <div className="absolute top-0 right-0 p-8 opacity-5">
                    <span className="text-8xl">{game?.icon}</span>
                </div>

                <div className="flex items-center gap-4">
                    <div className="w-12 h-12 bg-kaspa-primary/10 rounded-xl flex items-center justify-center text-2xl border border-kaspa-primary/20">
                        {game?.icon}
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
                    {/* Players Area */}
                    <div className="card flex items-center justify-between p-8 bg-gradient-to-r from-kaspa-dark to-kaspa-card">
                        <div className="text-center">
                            <div className="w-16 h-16 bg-kaspa-border rounded-full mx-auto mb-3 flex items-center justify-center border-2 border-kaspa-primary/30">👤</div>
                            <span className="font-bold block">{match.player_a_faceit_nickname}</span>
                            <span className="text-[9px] text-kaspa-primary uppercase font-black">{t('match.challenger')}</span>
                            <div className={`mt-2 h-1 w-full rounded-full ${match.player_a_deposit_tx_hash ? 'bg-kaspa-primary' : 'bg-gray-700'}`} />
                        </div>

                        <div className="text-center px-4">
                            <span className="text-4xl font-black italic opacity-20">VS</span>
                        </div>

                        <div className="text-center">
                            {match.player_b_faceit_nickname ? (
                                <>
                                    <div className="w-16 h-16 bg-kaspa-border rounded-full mx-auto mb-3 flex items-center justify-center border-2 border-kaspa-primary/30">👤</div>
                                    <span className="font-bold block">{match.player_b_faceit_nickname}</span>
                                    <span className="text-[9px] text-blue-400 uppercase font-black">{t('match.opponent')}</span>
                                    <div className={`mt-2 h-1 w-full rounded-full ${match.player_b_deposit_tx_hash ? 'bg-kaspa-primary' : 'bg-gray-700'}`} />
                                </>
                            ) : (
                                <div className="w-16 h-16 bg-kaspa-dark border-2 border-dashed border-kaspa-border rounded-full mx-auto mb-3 flex items-center justify-center text-gray-600 italic text-xl">?</div>
                            )}
                        </div>
                    </div>

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
