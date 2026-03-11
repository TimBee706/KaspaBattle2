import React, { useEffect, useState } from 'react';
import { useParams, useNavigate } from 'react-router-dom';
import { useWalletStore } from '../stores/useWalletStore';
import { formatKas } from '../utils/format';
import { useTranslation } from 'react-i18next';
import { useEscrowDeposit } from '../hooks/useEscrowDeposit';
import { useMatchPolling } from '../hooks/useMatchPolling';
import { useMatchStore } from '../stores/useMatchStore';
import { useAuthStore } from '../stores/useAuthStore';

export const EscrowPage: React.FC = () => {
    const { lobbyId } = useParams<{ lobbyId: string }>();
    const navigate = useNavigate();
    const { t } = useTranslation();
    
    const { user } = useAuthStore();
    const { balanceSompi } = useWalletStore();
    useMatchPolling(lobbyId ?? null); // Polles the full match detail
    
    const { currentMatch, isLoading, error: matchError } = useMatchStore();
    const { executeDeposit, isDepositing, depositTxHash } = useEscrowDeposit();
    const [localError, setLocalError] = useState<string | null>(null);

    // Auto-redirect when deposit tx hash comes in
    useEffect(() => {
        if (depositTxHash && lobbyId) {
            // Short delay to let the user see the success checkmark briefly if we had a dedicated success state, 
            // but for simplicity we just redirect back to match page
            const timer = setTimeout(() => {
                navigate(`/match/${lobbyId}`);
            }, 1000);
            return () => clearTimeout(timer);
        }
    }, [depositTxHash, lobbyId, navigate]);

    if (matchError || localError) {
        return (
            <div className="container mx-auto px-4 py-12 max-w-2xl text-center">
                <div className="p-4 bg-red-900/20 border border-red-500/30 rounded-xl text-red-500 font-bold mb-4">
                    ❌ {localError || matchError}
                </div>
                <button onClick={() => navigate('/lobby')} className="text-gray-500 hover:text-white uppercase tracking-widest text-xs font-bold">
                    {t('common.back', 'Zurück zur Übersicht')}
                </button>
            </div>
        );
    }

    if (isLoading && !currentMatch) {
         return (
            <div className="flex items-center justify-center min-h-[60vh]">
                <div className="animate-spin rounded-full h-12 w-12 border-t-2 border-b-2 border-kaspa-primary"></div>
            </div>
        );
    }

    if (!currentMatch) {
        return null;
    }

    const wagerAmountSompi = currentMatch.wager_amount_sompi || (currentMatch as any).stake_kas || 0;
    const hasEnoughBalance = (balanceSompi || 0) >= wagerAmountSompi;

    // Determine player role
    const isPlayerA = user?.faceit_id === currentMatch.player_a_faceit_id;
    const playerRole = isPlayerA ? 'A' : 'B';

    const handleDepositClick = async () => {
        setLocalError(null);
        try {
            await executeDeposit(playerRole);
        } catch (err: any) {
            setLocalError(err.message || "Einzahlung fehlgeschlagen");
        }
    };

    return (
        <div className="container mx-auto px-4 py-12 max-w-2xl">
            <div className="card border-2 border-kaspa-primary/30 p-8 animate-in fade-in zoom-in-95 duration-500">
                <div className="text-center mb-8">
                    <div className="w-20 h-20 bg-kaspa-primary/20 text-kaspa-primary rounded-full flex items-center justify-center text-4xl mx-auto mb-4">
                        {depositTxHash ? '✓' : '⚔️'}
                    </div>
                    <h1 className="text-3xl font-black uppercase tracking-tight mb-2">
                         {depositTxHash ? t('deposit.success_title', 'Erfolgreich!') : t('escrow.title', 'Challenge erstellt!')}
                    </h1>
                    <p className="text-gray-400">
                        {depositTxHash ? t('deposit.success_info', 'Einzahlung bestätigt. Weiterleitung...') : t('escrow.subtitle', 'Zahle deinen Einsatz ein, um die Challenge zu aktivieren.')}
                    </p>
                </div>

                <div className="space-y-6">
                    <div className="bg-kaspa-dark/50 rounded-2xl p-6 border border-kaspa-border">
                        <div className="mb-6">
                            <label className="text-[10px] text-gray-500 uppercase font-black block mb-2 tracking-widest">
                                {t('escrow.address', 'Escrow-Adresse')}
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
                                    title={t('common.copy', 'Kopieren')}
                                >
                                    📋
                                </button>
                            </div>
                        </div>

                        <div className="grid grid-cols-2 gap-4">
                            <div className="bg-kaspa-card p-4 rounded-xl border border-kaspa-border">
                                <span className="text-[10px] text-gray-500 uppercase font-black block mb-1">
                                    {t('escrow.amount', 'Einsatz')}
                                </span>
                                <span className="text-xl font-black text-white">
                                    {formatKas(wagerAmountSompi)} KAS
                                </span>
                            </div>
                            <div className="bg-kaspa-card p-4 rounded-xl border border-kaspa-border">
                                <span className="text-[10px] text-gray-500 uppercase font-black block mb-1">
                                    {t('escrow.status', 'Status')}
                                </span>
                                <span className="text-kaspa-primary font-bold">
                                    {depositTxHash ? `✓ ${t('deposit.success_title', 'Erfolgreich')}` : `⏳ ${t('escrow.pending', 'Warten...')}`}
                                </span>
                            </div>
                        </div>
                    </div>

                    {!hasEnoughBalance && !depositTxHash && (
                        <div className="p-4 bg-red-900/20 border border-red-500/30 rounded-xl flex gap-3 items-center">
                            <span className="text-2xl">⚠️</span>
                            <p className="text-red-400 text-xs font-bold">
                                {t('escrow.low_balance', 'Nicht genügend Guthaben vorhanden.')}
                            </p>
                        </div>
                    )}

                    <button
                        onClick={handleDepositClick}
                        disabled={!hasEnoughBalance || isDepositing || !!depositTxHash}
                        className="w-full bg-kaspa-primary hover:bg-kaspa-secondary text-kaspa-dark h-14 rounded-2xl font-black uppercase tracking-tight text-lg shadow-xl shadow-kaspa-primary/20 transition-all active:scale-95 disabled:opacity-50 disabled:grayscale"
                    >
                        {isDepositing || depositTxHash ? '...' : t('escrow.deposit_now', '💰 Jetzt einzahlen')}
                    </button>

                    <div className="text-center">
                        <button
                            onClick={() => navigate(depositTxHash ? `/match/${lobbyId}` : '/lobby')}
                            className="text-gray-500 hover:text-white text-xs font-bold uppercase tracking-widest transition-colors"
                        >
                            {depositTxHash ? t('common.go_to_match', 'Zum Match') : t('common.back', 'Zurück zur Übersicht')}
                        </button>
                    </div>
                </div>

                {!depositTxHash && (
                    <div className="mt-8 pt-8 border-t border-kaspa-border/50 text-center">
                        <p className="text-[10px] text-gray-500 uppercase font-black tracking-widest leading-relaxed">
                            {t('escrow.disclaimer', 'Hinweis: Nach der Einzahlung wird die Challenge in der Lobby veröffentlicht.')}
                        </p>
                    </div>
                 )}
            </div>
        </div>
    );
};
