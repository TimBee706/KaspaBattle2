import React, { useEffect, useState } from 'react';
import { useParams, useNavigate } from 'react-router-dom';
import { useLobby } from '../hooks/useLobby';
import { useWalletStore } from '../stores/useWalletStore';
import { formatKas } from '../utils/format';
import { useTranslation } from 'react-i18next';

export const EscrowPage: React.FC = () => {
    const { lobbyId } = useParams<{ lobbyId: string }>();
    const navigate = useNavigate();
    const { t } = useTranslation();
    const { balanceSompi } = useWalletStore();
    const { handleLobbyClick, selectedLobby, isCreating: isProcessing } = useLobby();
    const [error, setError] = useState<string | null>(null);

    useEffect(() => {
        if (lobbyId) {
            handleLobbyClick(lobbyId);
        }
    }, [lobbyId, handleLobbyClick]);

    const lobby = selectedLobby;
    const wagerAmountSompi = lobby?.wager_amount_sompi || 0;
    const hasEnoughBalance = (balanceSompi || 0) >= wagerAmountSompi;

    const handleDeposit = async () => {
        if (!lobby) return;

        setError(null);
        try {
            // Die Logik für die vollständige Einzahlung ist nun im DepositConfirmModal via MatchDetailView oder direkt
            // Wir triggern entweder einen Context/Store oder verweisen auf MatchDetailView
            setError("Einzahlung bitte über die Match-Detailansicht (Dashboard) starten.");
        } catch (err: any) {
            setError(err.message || "Einzahlung fehlgeschlagen");
        }
    };

    if (!lobby) {
        return (
            <div className="flex items-center justify-center min-h-[60vh]">
                <div className="animate-spin rounded-full h-12 w-12 border-t-2 border-b-2 border-kaspa-primary"></div>
            </div>
        );
    }

    return (
        <div className="container mx-auto px-4 py-12 max-w-2xl">
            <div className="card border-2 border-kaspa-primary/30 p-8 animate-in fade-in zoom-in-95 duration-500">
                <div className="text-center mb-8">
                    <div className="w-20 h-20 bg-kaspa-primary/20 text-kaspa-primary rounded-full flex items-center justify-center text-4xl mx-auto mb-4">
                        ⚔️
                    </div>
                    <h1 className="text-3xl font-black uppercase tracking-tight mb-2">
                        {t('escrow.title', 'Challenge erstellt!')}
                    </h1>
                    <p className="text-gray-400">
                        {t('escrow.subtitle', 'Zahle deinen Einsatz ein, um die Challenge zu aktivieren.')}
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
                                    {lobby.escrow_address}
                                </div>
                                <button
                                    onClick={() => {
                                        navigator.clipboard.writeText(lobby.escrow_address);
                                        // Optional: toast notification
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
                                    ⏳ {t('escrow.pending', 'Warten...')}
                                </span>
                            </div>
                        </div>
                    </div>

                    {!hasEnoughBalance && (
                        <div className="p-4 bg-red-900/20 border border-red-500/30 rounded-xl flex gap-3 items-center">
                            <span className="text-2xl">⚠️</span>
                            <p className="text-red-400 text-xs font-bold">
                                {t('escrow.low_balance', 'Nicht genügend Guthaben vorhanden.')}
                            </p>
                        </div>
                    )}

                    {error && (
                        <div className="p-4 bg-red-900/20 border border-red-500/30 rounded-xl text-red-500 text-center text-xs font-bold">
                            ❌ {error}
                        </div>
                    )}

                    <button
                        onClick={handleDeposit}
                        disabled={!hasEnoughBalance || isProcessing}
                        className="w-full bg-kaspa-primary hover:bg-kaspa-secondary text-kaspa-dark h-14 rounded-2xl font-black uppercase tracking-tight text-lg shadow-xl shadow-kaspa-primary/20 transition-all active:scale-95 disabled:opacity-50 disabled:grayscale"
                    >
                        {isProcessing ? '...' : t('escrow.deposit_now', '💰 Jetzt einzahlen')}
                    </button>

                    <div className="text-center">
                        <button
                            onClick={() => navigate('/lobby')}
                            className="text-gray-500 hover:text-white text-xs font-bold uppercase tracking-widest transition-colors"
                        >
                            {t('common.back', 'Zurück zur Übersicht')}
                        </button>
                    </div>
                </div>

                <div className="mt-8 pt-8 border-t border-kaspa-border/50 text-center">
                    <p className="text-[10px] text-gray-500 uppercase font-black tracking-widest leading-relaxed">
                        {t('escrow.disclaimer', 'Hinweis: Nach der Einzahlung wird die Challenge in der Lobby veröffentlicht.')}
                    </p>
                </div>
            </div>
        </div>
    );
};
