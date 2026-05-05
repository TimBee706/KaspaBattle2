import React, { useState } from 'react';
import { type BattleMatch, getMatchMode, getMatchStakeSompi, getMatchCreatedAt } from '../api/types';
import { useAuthStore } from '../stores/useAuthStore';
import apiClient from '../api/client';
import { useNavigate } from 'react-router-dom';
import { useTranslation } from 'react-i18next';

function MatchStatusBadge({ status }: { status: string }) {
    let color = '#6b7280'; // gray
    if (status === 'OPEN' || status === 'WAITING_FOR_DEPOSITS') color = '#fbbf24'; // yellow
    if (status === 'FUNDED' || status === 'MATCH_READY' || status === 'IN_GAME' || status === 'GAME_ID_INPUT') color = '#34d399'; // green
    if (status === 'COMPLETED' || status === 'FINISHED_FACEIT' || status === 'RESOLVED' || status === 'PAID_OUT' || status === 'READY_FOR_PAYOUT') color = '#60a5fa'; // blue
    if (status === 'CANCELLED' || status === 'DISPUTED' || status === 'REFUNDED') color = '#f87171'; // red

    return (
        <span
            style={{ color, borderColor: color + '44', backgroundColor: color + '18' }}
            className="inline-flex items-center gap-1.5 px-2.5 py-0.5 rounded-full text-[10px] font-bold tracking-wider uppercase border shrink-0"
        >
            <span
                style={{ backgroundColor: color }}
                className="w-1.5 h-1.5 rounded-full animate-pulse"
            />
            {status ? status.replace(/_/g, ' ') : 'UNKNOWN'}
        </span>
    );
}

export const LobbyTable: React.FC<{ matches: BattleMatch[], title?: string, onLobbyClick?: (id: string) => void }> = ({ matches, title, onLobbyClick }) => {
    const { user, testMode, isFaceitConnected } = useAuthStore();
    const kaspaAddress = user?.kaspa_address || null;
    const [loading, setLoading] = useState<string | null>(null);
    const navigate = useNavigate();
    const { t } = useTranslation();

    const join = async (id: string) => {
        setLoading(id);
        try {
            await apiClient.post(`/matches/${id}/accept`);
            navigate(`/escrow/${id}`);
        } catch (e) { console.error(e); }
        finally { setLoading(null); }
    };

    return (
        <div className="mb-8 p-6 glass-panel rounded-2xl border border-kaspa-primary/20 shadow-glow-primary">
            {title && <h2 className="text-xl font-black mb-6 text-emerald-400 uppercase tracking-tighter pl-2">{title}</h2>}
            <div className="flex flex-col gap-3">
                {matches.length === 0 && <p className="text-slate-500 italic pl-2">{t('lobby.no_entries')}</p>}
                {matches.map((m, index) => {
                    const matchTitle = `${m.player_a_faceit_nickname} vs ${m.player_b_faceit_nickname || 'TBD'}`;
                    const createdAt = getMatchCreatedAt(m);
                    
                    return (
                        <div
                            key={`${m.id}-${index}`}
                            onClick={() => onLobbyClick?.(m.id)}
                            role="button"
                            tabIndex={0}
                            onKeyDown={(e) => e.key === 'Enter' && onLobbyClick?.(m.id)}
                            className="group bg-slate-900/60 border border-slate-700/50 p-4 rounded-xl flex flex-col md:flex-row justify-between items-start md:items-center gap-4 hover:border-emerald-500/50 hover:bg-slate-800/80 transition-all cursor-pointer select-none active:scale-[0.99]"
                        >
                            {/* Left side */}
                            <div className="flex-1 min-w-0">
                                <div className="flex items-center gap-3 mb-1">
                                    <h3 className="font-bold text-white text-lg truncate group-hover:text-emerald-400 transition-colors">
                                        {matchTitle}
                                    </h3>
                                    <MatchStatusBadge status={m.status} />
                                </div>
                                <div className="flex items-center gap-3 text-xs text-slate-500 font-bold">
                                    <span className="uppercase tracking-wider">{(m.game_id || 'CS2').toUpperCase()}</span>
                                    <span>•</span>
                                    <span className="uppercase tracking-wider">{getMatchMode(m)}</span>
                                    {createdAt && (
                                        <>
                                            <span>•</span>
                                            <span>{new Date(createdAt).toLocaleString()}</span>
                                        </>
                                    )}
                                </div>
                            </div>

                            {/* Right side */}
                            <div className="flex items-center gap-4 md:gap-6 w-full md:w-auto mt-2 md:mt-0 pt-3 md:pt-0 border-t md:border-0 border-slate-700/30">
                                <div className="text-right shrink-0">
                                    <div className="text-emerald-400 text-xl font-black flex items-baseline gap-1 justify-end">
                                        {(getMatchStakeSompi(m) / 100_000_000).toLocaleString('de-DE', { minimumFractionDigits: 2 })} <span className="text-sm font-bold text-emerald-400/80">KAS</span>
                                    </div>
                                    <div className="text-xs text-slate-500 font-bold uppercase tracking-widest mt-0.5">
                                        STAKE PER PLAYER
                                    </div>
                                </div>
                                {m.creator_user_id !== user?.id && m.opponent_user_id !== user?.id && m.status === 'OPEN' && !m.opponent_user_id && (
                                    <button
                                        onClick={(e) => {
                                            e.stopPropagation();
                                            join(m.id);
                                        }}
                                        disabled={loading === m.id || !kaspaAddress || (!isFaceitConnected && !testMode)}
                                        className="bg-emerald-600 hover:bg-emerald-500 px-6 py-2.5 rounded-lg font-black uppercase tracking-tighter disabled:opacity-30 disabled:hover:bg-emerald-600 transition-all shadow-lg shadow-emerald-900/20 active:scale-95"
                                    >
                                        {loading === m.id ? '...' : t('lobby.join')}
                                    </button>
                                )}
                            </div>
                        </div>
                    );
                })}
            </div>
            {!kaspaAddress && <p className="text-red-400 mt-2 text-sm">{t('lobby.connect_wallet_join')}</p>}
            {kaspaAddress && isFaceitConnected && !testMode && (
                <p className="text-green-400 mt-2 text-sm flex items-center gap-2">
                    <span className="inline-block w-2 h-2 bg-green-500 rounded-full"></span>
                    {t('lobby.faceit_connected_as')} <strong>{user?.faceit_nickname}</strong>
                </p>
            )}
            {kaspaAddress && !isFaceitConnected && !testMode && <p className="text-yellow-400 mt-2 text-sm">{t('lobby.connect_faceit_join')}</p>}
            {testMode && <p className="text-blue-400 mt-2 text-sm italic">{t('lobby.test_mode_notice')}</p>}
        </div>
    );
};
