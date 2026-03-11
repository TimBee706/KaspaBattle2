import React, { useState } from 'react';
import type { BattleMatch } from '../api/types';
import { useAuthStore } from '../stores/useAuthStore';
import apiClient from '../api/client';
import { useNavigate } from 'react-router-dom';

export const LobbyTable: React.FC<{ matches: BattleMatch[], title: string, isMyLobbies?: boolean, onLobbyClick?: (id: string) => void }> = ({ matches, title, isMyLobbies, onLobbyClick }) => {
    const { user, testMode, isFaceitConnected } = useAuthStore();
    const kaspaAddress = user?.kaspa_address || null;
    const [loading, setLoading] = useState<string | null>(null);
    const navigate = useNavigate();

    const join = async (id: string) => {
        setLoading(id);
        try {
            await apiClient.post(`/challenges/${id}/join`);
            navigate(`/escrow/${id}`);
        } catch (e) { console.error(e); }
        finally { setLoading(null); }
    };

    return (
        <div className="mb-8 p-4 bg-slate-800/40 rounded-2xl border border-slate-700/50">
            <h2 className="text-xl font-black mb-6 text-emerald-400 uppercase tracking-tighter pl-2">{title}</h2>
            <div className="flex flex-col gap-3">
                {matches.length === 0 && <p className="text-slate-500 italic pl-2">Keine Einträge.</p>}
                {matches.map(m => (
                    <div
                        key={m.id}
                        onClick={() => onLobbyClick?.(m.id)}
                        role="button"
                        tabIndex={0}
                        onKeyDown={(e) => e.key === 'Enter' && onLobbyClick?.(m.id)}
                        className="group bg-slate-900/60 border border-slate-700/50 p-5 rounded-xl flex justify-between items-center text-white hover:border-emerald-500/50 hover:bg-slate-800/80 transition-all cursor-pointer select-none active:scale-[0.99]"
                    >
                        <div className="flex flex-col">
                            <span className="font-black text-white tracking-tight group-hover:text-emerald-400 transition-colors uppercase">
                                {m.game_id || 'CS2'} | {m.match_mode || (m as any).mode || 'BO1'}
                            </span>
                            <span className="text-slate-500 text-[10px] font-bold uppercase tracking-widest mt-1">Status: {m.status}</span>
                        </div>
                        <div className="flex items-center gap-6">
                            <span className="font-black text-xl text-emerald-400 tracking-tighter">
                                {((m.wager_amount_sompi || (m as any).stake_kas || 0) / 100_000_000).toLocaleString('de-DE', { minimumFractionDigits: 2 })} KAS
                            </span>
                            {!isMyLobbies && m.status === 'OPEN' && (
                                <button
                                    onClick={(e) => {
                                        e.stopPropagation();
                                        join(m.id);
                                    }}
                                    disabled={loading === m.id || !kaspaAddress || (!isFaceitConnected && !testMode)}
                                    className="bg-emerald-600 hover:bg-emerald-500 px-6 py-2.5 rounded-lg font-black uppercase tracking-tighter disabled:opacity-30 disabled:hover:bg-emerald-600 transition-all shadow-lg shadow-emerald-900/20 active:scale-95"
                                >
                                    {loading === m.id ? '...' : 'Join'}
                                </button>
                            )}
                        </div>
                    </div>
                ))}
            </div>
            {!isMyLobbies && !kaspaAddress && <p className="text-red-400 mt-2 text-sm">Wallet verbinden zum Beitreten!</p>}
            {!isMyLobbies && kaspaAddress && isFaceitConnected && !testMode && (
                <p className="text-green-400 mt-2 text-sm flex items-center gap-2">
                    <span className="inline-block w-2 h-2 bg-green-500 rounded-full"></span>
                    Mit FACEIT verbunden als <strong>{user?.faceit_nickname}</strong>
                </p>
            )}
            {!isMyLobbies && kaspaAddress && !isFaceitConnected && !testMode && <p className="text-yellow-400 mt-2 text-sm">FACEIT verbinden zum Beitreten!</p>}
            {testMode && <p className="text-blue-400 mt-2 text-sm italic">Test-Modus: Beitreten ohne FACEIT möglich.</p>}
        </div>
    );
};
