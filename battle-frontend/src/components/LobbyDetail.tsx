import React from 'react';
import type { BattleMatch } from '../api/types';
import { useTranslation } from 'react-i18next';
import { useAuthStore } from '../stores/useAuthStore';

interface LobbyDetailProps {
    lobby: BattleMatch;
    onClose: () => void;
    onJoin?: (id: string, stake: number) => void;
    isJoining?: boolean;
}

export const LobbyDetail: React.FC<LobbyDetailProps> = ({ lobby, onClose, onJoin, isJoining }) => {
    const { t } = useTranslation();
    const { user } = useAuthStore();

    const wagerKas = (lobby.wager_amount_sompi || (lobby as any).stake_kas || 0) / 100_000_000;

    const getStatusColor = (status: string) => {
        switch (status) {
            case 'OPEN': return 'bg-emerald-500/20 text-emerald-400 border-emerald-500/50';
            case 'IN_PROGRESS': return 'bg-yellow-500/20 text-yellow-400 border-yellow-500/50';
            case 'FINISHED': return 'bg-slate-500/20 text-slate-400 border-slate-500/50';
            default: return 'bg-slate-700 text-slate-300 border-slate-600';
        }
    };

    const shortenAddr = (addr: string) => addr ? `${addr.slice(0, 10)}...${addr.slice(-6)}` : '???';

    return (
        <div className="fixed inset-0 bg-black/80 backdrop-blur-sm flex items-center justify-center z-[100] p-4 animate-in fade-in duration-200">
            <div className="bg-slate-900 border border-slate-700 rounded-2xl w-full max-w-lg shadow-2xl overflow-hidden animate-in zoom-in-95 duration-200">
                {/* Header */}
                <div className="p-6 border-b border-slate-800 flex justify-between items-center bg-slate-800/50">
                    <div>
                        <h2 className="text-2xl font-black text-white uppercase tracking-tighter">
                            {lobby.game_id || 'CS2'} Match
                        </h2>
                        <p className="text-slate-400 text-xs font-bold uppercase tracking-widest mt-1">
                            ID: {lobby.id.slice(0, 8)}
                        </p>
                    </div>
                    <button
                        onClick={onClose}
                        className="w-10 h-10 flex items-center justify-center rounded-full hover:bg-slate-700 text-slate-400 hover:text-white transition-colors"
                    >
                        ✕
                    </button>
                </div>

                {/* Content */}
                <div className="p-8 space-y-8">
                    <div className="grid grid-cols-2 gap-8">
                        <div>
                            <label className="text-[10px] font-black text-slate-500 uppercase tracking-widest block mb-2">{t('lobby.mode', 'Modus')}</label>
                            <div className="text-white font-bold">{lobby.match_mode || (lobby as any).mode || 'BO1'}</div>
                        </div>
                        <div>
                            <label className="text-[10px] font-black text-slate-500 uppercase tracking-widest block mb-2">{t('lobby.stake', 'Einsatz')}</label>
                            <div className="text-emerald-400 font-black text-xl">{wagerKas.toLocaleString('de-DE', { minimumFractionDigits: 2 })} KAS</div>
                        </div>
                    </div>

                    <div>
                        <label className="text-[10px] font-black text-slate-500 uppercase tracking-widest block mb-3">{t('lobby.players', 'Spieler')}</label>
                        <div className="space-y-3">
                            <div className="flex items-center justify-between bg-slate-800/50 p-3 rounded-lg border border-slate-700/50">
                                <span className="text-slate-300 text-sm font-mono">{shortenAddr(lobby.player_a_kas_address || '')}</span>
                                <span className="text-[10px] bg-blue-500/20 text-blue-400 px-2 py-1 rounded font-black uppercase tracking-tighter">Creator</span>
                            </div>
                            {lobby.player_b_kas_address ? (
                                <div className="flex items-center justify-between bg-slate-800/50 p-3 rounded-lg border border-slate-700/50">
                                    <span className="text-slate-300 text-sm font-mono">{shortenAddr(lobby.player_b_kas_address || '')}</span>
                                    <span className="text-[10px] bg-slate-700 text-slate-400 px-2 py-1 rounded font-black uppercase tracking-tighter">Opponent</span>
                                </div>
                            ) : (
                                <div className="p-3 border border-dashed border-slate-700 rounded-lg text-slate-500 text-sm italic text-center">
                                    Warten auf Gegner...
                                </div>
                            )}
                        </div>
                    </div>

                    <div className="flex items-center justify-between">
                        <span className={`px-3 py-1 rounded-full text-[10px] font-black uppercase tracking-widest border ${getStatusColor(lobby.status)}`}>
                            {lobby.status}
                        </span>
                        <span className="text-slate-500 text-[10px] uppercase font-bold tracking-widest italic font-mono">
                            {new Date(lobby.created_at || (lobby as any).createdAt).toLocaleDateString()}
                        </span>
                    </div>
                </div>

                {/* Footer Actions */}
                <div className="p-6 bg-slate-800/30 border-t border-slate-800 flex gap-4">
                    <button
                        onClick={onClose}
                        className="flex-1 py-3 px-6 bg-slate-700 hover:bg-slate-600 text-white font-bold rounded-xl transition-all"
                    >
                        {t('common.close', 'Schließen')}
                    </button>
                    {lobby.status === 'OPEN' && !!user && user.id !== lobby.creator_user_id && user.id !== lobby.opponent_user_id && onJoin && (
                        <button
                            onClick={() => onJoin(lobby.id, wagerKas)}
                            disabled={isJoining}
                            className="flex-[2] py-3 px-6 bg-emerald-600 hover:bg-emerald-500 disabled:bg-slate-800 text-white font-black rounded-xl transition-all shadow-lg shadow-emerald-900/20"
                        >
                            {isJoining ? 'Lädt...' : t('lobby.join', 'Herausforderung annehmen')}
                        </button>
                    )}
                </div>
            </div>
        </div>
    );
};
