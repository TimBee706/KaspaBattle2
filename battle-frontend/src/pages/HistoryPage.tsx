import React, { useEffect } from 'react';
import { useLobbyStore } from '../stores/useLobbyStore';
import { useTranslation } from 'react-i18next';
import { useNavigate } from 'react-router-dom';
import { explorerTxUrl } from '../utils/format';
import { getMatchCreatedAt } from '../api/types';

export const HistoryPage: React.FC = () => {
    const { history, setHistory } = useLobbyStore();
    const { t } = useTranslation();
    const navigate = useNavigate();

    useEffect(() => {
        fetch('/api/v1/history').then(r => r.json()).then(data => setHistory(data || [])).catch(console.error);
    }, [setHistory]);

    return (
        <div>
            {/* Header — mirrors LobbyPage exactly */}
            <div className="flex flex-col md:flex-row md:justify-between items-start md:items-center gap-6 mb-12">
                <div>
                    <h1 className="text-4xl font-black text-white uppercase tracking-tighter mb-2">{t('navigation.history')}</h1>
                    <p className="text-slate-500 text-sm font-bold uppercase tracking-widest">{t('lobby.active_matches')}</p>
                </div>
            </div>

            {/* Section box — same style as LobbyTable */}
            <div className="mb-8 p-6 glass-panel rounded-2xl border border-kaspa-primary/15">
                <div className="flex flex-col gap-3">
                    {history.length === 0 && <p className="text-slate-500 italic pl-2">{t('common.no_matches')}</p>}
                    {history.map(m => (
                        <div
                            key={m.id}
                            onClick={() => navigate(`/match/${m.id}`)}
                            className="group bg-slate-900/60 border border-slate-700/50 p-5 rounded-xl flex justify-between items-center hover:border-emerald-500/50 hover:bg-slate-800/80 transition-all cursor-pointer select-none active:scale-[0.99]"
                        >
                            <div className="flex flex-col">
                                <span className="font-black uppercase tracking-tight group-hover:text-emerald-400 transition-colors">
                                    {m.game_id} | {m.match_mode}
                                </span>
                                <span className="text-slate-500 text-[10px] font-bold uppercase tracking-widest mt-1">
                                    {new Date(getMatchCreatedAt(m)).toLocaleDateString()}
                                </span>
                            </div>

                            <div className="flex items-center gap-6">
                                <span className="font-black text-xl text-emerald-400 tracking-tighter">
                                    {(m.wager_amount_sompi / 100_000_000).toLocaleString('de-DE', { minimumFractionDigits: 2 })} KAS
                                </span>
                                {m.payout_tx_hash && (
                                    <a
                                        href={explorerTxUrl(m.payout_tx_hash)}
                                        target="_blank"
                                        rel="noreferrer"
                                        onClick={(e) => e.stopPropagation()}
                                        className="text-kaspa-primary hover:underline text-[10px] font-black uppercase tracking-widest shrink-0"
                                    >
                                        TX ↗
                                    </a>
                                )}
                            </div>
                        </div>
                    ))}
                </div>
            </div>
        </div>
    );
};
