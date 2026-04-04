import React, { useEffect, useState } from 'react';
import { useParams, useNavigate } from 'react-router-dom';
import { useWalletStore } from '../stores/useWalletStore';
import { formatKas } from '../utils/format';
import { useTranslation } from 'react-i18next';
import { useEscrowDeposit } from '../hooks/useEscrowDeposit';
import { useMatchPolling } from '../hooks/useMatchPolling';
import { usePaymentStatus } from '../hooks/usePaymentStatus';
import { useMatchStore } from '../stores/useMatchStore';
import { useAuthStore } from '../stores/useAuthStore';
import { SOMPI_PER_KAS } from '../config/constants';
import {
    getLobbyRole,
    getPlayerRoleForLobby,
    hasPlayerDeposited,
    isLocalDepositForIdentity,
} from '../domain/lobby';

// ─── Helper: deposit progress bar ─────────────────────────────────────────────
interface DepositCardProps {
    label: string;
    info: { paid: boolean; confirmed_sompi: number; min_confirmations: number; payment_count: number };
    required: number;
    minConf: number;
    isCurrentPlayer: boolean;
}

const DepositCard: React.FC<DepositCardProps> = ({ label, info, required, minConf, isCurrentPlayer }) => {
    const { t } = useTranslation();
    const pct = Math.min(100, required > 0 ? Math.round((info.confirmed_sompi / required) * 100) : 0);
    const confPct = Math.min(100, minConf > 0 ? Math.round((info.min_confirmations / minConf) * 100) : 0);

    return (
        <div className={`rounded-xl p-4 border transition-all duration-500 ${
            info.paid
                ? 'bg-green-900/20 border-green-500/40'
                : 'bg-kaspa-card border-kaspa-border'
        }`}>
            {/* Header */}
            <div className="flex items-center justify-between mb-3">
                <span className="text-[10px] font-black uppercase tracking-widest text-gray-500">
                    {label}
                    {isCurrentPlayer && (
                        <span className="ml-2 text-kaspa-primary">{t('escrow.me_suffix')}</span>
                    )}
                </span>
                <span className={`text-xs font-bold ${info.paid ? 'text-green-400' : 'text-gray-500'}`}>
                    {info.paid ? t('escrow.status_confirmed') : info.payment_count > 0 ? t('escrow.status_confirmations') : t('escrow.status_waiting')}
                </span>
            </div>

            {/* Amount bar */}
            <div className="mb-2">
                <div className="flex justify-between text-[10px] text-gray-500 mb-1">
                    <span>{formatKas(info.confirmed_sompi)} KAS</span>
                    <span>{formatKas(required)} KAS</span>
                </div>
                <div className="h-1.5 bg-kaspa-dark rounded-full overflow-hidden">
                    <div
                        className={`h-full rounded-full transition-all duration-700 ${info.paid ? 'bg-green-500' : 'bg-kaspa-primary'}`}
                        style={{ width: `${pct}%` }}
                    />
                </div>
            </div>

            {/* Confirmation bar (only visible when UTXOs detected but not yet confirmed) */}
            {info.payment_count > 0 && !info.paid && (
                <div>
                    <div className="flex justify-between text-[10px] text-gray-600 mb-1">
                        <span>{info.min_confirmations}/{minConf} Confirmations</span>
                        <span>{confPct}%</span>
                    </div>
                    <div className="h-1 bg-kaspa-dark rounded-full overflow-hidden">
                        <div
                            className="h-full bg-yellow-500/60 rounded-full transition-all duration-700"
                            style={{ width: `${confPct}%` }}
                        />
                    </div>
                </div>
            )}
        </div>
    );
};

// ─── Main Page ─────────────────────────────────────────────────────────────────
export const EscrowPage: React.FC = () => {
    const { lobbyId } = useParams<{ lobbyId: string }>();
    const navigate = useNavigate();
    const { t } = useTranslation();

    const { user } = useAuthStore();
    const { balanceSompi } = useWalletStore();

    // Match polling: real-time WS + HTTP fallback
    useMatchPolling(lobbyId ?? null);
    // Payment confirmation polling: 3s while AWAITING_FUNDING
    usePaymentStatus(lobbyId ?? null);

    const { currentMatch, isLoading, error: matchError, paymentStatus, localDeposit } = useMatchStore();
    const { executeDeposit, isDepositing, depositTxHash } = useEscrowDeposit();
    const [localError, setLocalError] = useState<string | null>(null);

    const hasLocalDeposit = currentMatch
        ? isLocalDepositForIdentity(localDeposit, {
            matchId: currentMatch.id,
            userId: user?.id ?? null,
            walletAddress: user?.kaspa_address ?? null,
        })
        : false;

    useEffect(() => {
        const status = currentMatch?.status;
        const shouldRedirect =
            hasLocalDeposit || status === 'FUNDED' || status === 'LOCKED';
        if (!shouldRedirect || !lobbyId) return;

        const timer = setTimeout(() => navigate(`/match/${lobbyId}`), 1500);
        return () => clearTimeout(timer);
    }, [currentMatch?.status, hasLocalDeposit, lobbyId, navigate]);

    // ── Early returns ─────────────────────────────────────────────────────────
    if (matchError || localError) {
        return (
            <div className="container mx-auto px-4 py-12 max-w-2xl text-center">
                <div className="p-4 bg-red-900/20 border border-red-500/30 rounded-xl text-red-500 font-bold mb-4">
                    ❌ {localError || matchError}
                </div>
                <button onClick={() => navigate('/lobby')} className="text-gray-500 hover:text-white uppercase tracking-widest text-xs font-bold">
                    {t('common.back')}
                </button>
            </div>
        );
    }

    if (isLoading && !currentMatch) {
        return (
            <div className="flex items-center justify-center min-h-[60vh]">
                <div className="animate-spin rounded-full h-12 w-12 border-t-2 border-b-2 border-kaspa-primary" />
            </div>
        );
    }

    if (!currentMatch) return null;

    const wagerAmountSompi = currentMatch.wager_amount_sompi || (currentMatch as any).stake_kas || 0;
    const wagerKas = wagerAmountSompi / SOMPI_PER_KAS;
    const hasEnoughBalance = (balanceSompi || 0) >= wagerAmountSompi;

    // Determine player role — backend returns creator_user_id / opponent_user_id, not faceit fields
    const lobbyRole = getLobbyRole(currentMatch, user?.id ?? null);
    const playerRole = getPlayerRoleForLobby(currentMatch, user?.id ?? null);
    const iHavePaid = hasPlayerDeposited(paymentStatus, playerRole) || hasLocalDeposit;
    const successState = iHavePaid || currentMatch.status === 'FUNDED';

    const handleDepositClick = async () => {
        setLocalError(null);
        try {
            if (!playerRole) {
                throw new Error(t('match.accept_error')); // Or a better specific key if exists
            }
            await executeDeposit(playerRole);
        } catch (err: any) {
            setLocalError(err.message || t('common.error', { message: '' }));
        }
    };

    return (
        <div className="container mx-auto px-4 py-12 max-w-2xl">
            <div className="card border-2 border-kaspa-primary/30 p-8 animate-in fade-in zoom-in-95 duration-500">

                {/* Header */}
                <div className="text-center mb-8">
                    <div className={`w-20 h-20 rounded-full flex items-center justify-center text-4xl mx-auto mb-4 transition-all duration-500 ${
                        successState
                            ? 'bg-green-500/20 text-green-400'
                            : 'bg-kaspa-primary/20 text-kaspa-primary'
                    }`}>
                        {successState ? '✓' : '⚔️'}
                    </div>
                    <h1 className="text-3xl font-black uppercase tracking-tight mb-2">
                        {successState
                            ? t('deposit.success_title')
                            : t('escrow.title')}
                    </h1>
                    <p className="text-gray-400">
                        {successState
                            ? t('deposit.success_info')
                            : t('escrow.subtitle')}
                    </p>
                </div>

                <div className="space-y-6">
                    {/* Escrow Info */}
                    <div className="bg-kaspa-dark/50 rounded-2xl p-6 border border-kaspa-border">
                        <div className="mb-6">
                            <label className="text-[10px] text-gray-500 uppercase font-black block mb-2 tracking-widest">
                                {t('escrow.address')}
                            </label>
                            <div className="flex items-center gap-3">
                                <div className="bg-kaspa-dark border border-kaspa-border p-3 rounded-xl font-mono text-sm text-kaspa-primary flex-1 overflow-hidden text-ellipsis whitespace-nowrap">
                                    {currentMatch.escrow_address}
                                </div>
                                <button
                                    onClick={() => {
                                        if (currentMatch.escrow_address) navigator.clipboard.writeText(currentMatch.escrow_address);
                                    }}
                                    className="p-3 bg-kaspa-border hover:bg-gray-700 rounded-xl transition-colors text-white"
                                    title={t('common.copy')}
                                >
                                    📋
                                </button>
                            </div>
                        </div>

                        <div className="grid grid-cols-2 gap-4 mb-6">
                            <div className="bg-kaspa-card p-4 rounded-xl border border-kaspa-border">
                                <span className="text-[10px] text-gray-500 uppercase font-black block mb-1">
                                    {t('escrow.amount')}
                                </span>
                                <span className="text-xl font-black text-white">
                                    {wagerKas.toLocaleString()} KAS
                                </span>
                            </div>
                            <div className="bg-kaspa-card p-4 rounded-xl border border-kaspa-border">
                                <span className="text-[10px] text-gray-500 uppercase font-black block mb-1">
                                    {t('escrow.confirmations')}
                                </span>
                                <span className="text-xl font-black text-white">
                                    {paymentStatus ? `min. ${paymentStatus.min_confirmations_required}` : '10'}
                                </span>
                            </div>
                        </div>

                        {/* Live per-player deposit status */}
                        {paymentStatus ? (
                            <div className="space-y-3">
                                <p className="text-[10px] text-gray-500 uppercase font-black tracking-widest mb-2">
                                    — {t('match.status_actions')} —
                                </p>
                                <DepositCard
                                    label={currentMatch.player_a_faceit_nickname || t('match.challenger')}
                                    info={paymentStatus.playerA}
                                    required={paymentStatus.required_per_player_sompi}
                                    minConf={paymentStatus.min_confirmations_required}
                                    isCurrentPlayer={playerRole === 'A'}
                                />
                                <DepositCard
                                    label={currentMatch.player_b_faceit_nickname || t('match.opponent')}
                                    info={paymentStatus.playerB}
                                    required={paymentStatus.required_per_player_sompi}
                                    minConf={paymentStatus.min_confirmations_required}
                                    isCurrentPlayer={playerRole === 'B'}
                                />
                            </div>
                        ) : (
                            // Fallback before first poll response
                            <div className="bg-kaspa-card p-4 rounded-xl border border-kaspa-border text-center">
                                <span className="text-kaspa-primary font-bold text-sm">
                                    {depositTxHash && hasLocalDeposit ? `✓ ${t('deposit.success_title')}` : `⏳ ${t('escrow.pending')}`}
                                </span>
                            </div>
                        )}
                    </div>

                    {/* Balance warning */}
                    {!hasEnoughBalance && !iHavePaid && !!playerRole && (
                        <div className="p-4 bg-red-900/20 border border-red-500/30 rounded-xl flex gap-3 items-center">
                            <span className="text-2xl">⚠️</span>
                            <p className="text-red-400 text-xs font-bold">
                                {t('escrow.low_balance')}
                            </p>
                        </div>
                    )}

                    {/* Already paid hint */}
                    {playerRole && iHavePaid && !paymentStatus?.both_paid && (
                        <div className="p-4 bg-kaspa-primary/10 border border-kaspa-primary/30 rounded-xl flex gap-3 items-center">
                            <span className="text-2xl">⏳</span>
                            <p className="text-kaspa-primary text-xs font-bold">
                                {t('escrow.waiting_opponent')}
                            </p>
                        </div>
                    )}

                    {/* Deposit Button */}
                    <button
                        onClick={handleDepositClick}
                        disabled={!playerRole || !hasEnoughBalance || isDepositing || iHavePaid}
                        className="w-full bg-kaspa-primary hover:bg-kaspa-secondary text-kaspa-dark h-14 rounded-2xl font-black uppercase tracking-tight text-lg shadow-xl shadow-kaspa-primary/20 transition-all active:scale-95 disabled:opacity-50 disabled:grayscale"
                    >
                        {isDepositing
                            ? `⏳ ${t('deposit.signing')}`
                            : iHavePaid
                            ? `✅ ${t('deposit.success_title')}`
                            : t('escrow.deposit_now')}
                    </button>

                    <div className="text-center">
                        <button
                            onClick={() => navigate(successState ? `/match/${lobbyId}` : '/lobby')}
                            className="text-gray-500 hover:text-white text-xs font-bold uppercase tracking-widest transition-colors"
                        >
                            {successState ? t('common.go_to_match') : t('common.back')}
                        </button>
                    </div>
                </div>

                {/* Disclaimer */}
                {!iHavePaid && lobbyRole !== 'viewer' && (
                    <div className="mt-8 pt-8 border-t border-kaspa-border/50 text-center">
                        <p className="text-[10px] text-gray-500 uppercase font-black tracking-widest leading-relaxed">
                            {t('escrow.disclaimer')}
                        </p>
                    </div>
                )}
            </div>
        </div>
    );
};
