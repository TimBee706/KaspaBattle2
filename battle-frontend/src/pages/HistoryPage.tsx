import { useEffect, useState } from 'react';
import { getMyMatches } from '../api/matches';
import type { BattleMatch } from '../api/types';
import { MatchStatusBadge } from '../components/match/MatchStatusBadge';
import { formatKas } from '../utils/format';
import { SUPPORTED_GAMES } from '../config/constants';
import { Link } from 'react-router-dom';
import { faceitApi } from '../api/faceit';
import type { FaceitMatchHistoryItem } from '../api/types';

export function HistoryPage() {
    const [matches, setMatches] = useState<BattleMatch[]>([]);
    const [faceitMatches, setFaceitMatches] = useState<FaceitMatchHistoryItem[]>([]);
    const [loading, setLoading] = useState(true);
    const [activeTab, setActiveTab] = useState<'KASPA' | 'FACEIT'>('KASPA');

    useEffect(() => {
        Promise.all([
            getMyMatches().catch(() => ({ matches: [] })),
            faceitApi.getMatches().catch(() => ({ items: [] }))
        ]).then(([kaspaRes, faceitRes]) => {
            setMatches(kaspaRes.matches);
            // @ts-ignore - Die API Struktur muss evtl gecastet werden
            setFaceitMatches(faceitRes.items || []);
        }).finally(() => setLoading(false));
    }, []);

    return (
        <div className="max-w-5xl mx-auto space-y-8 animate-in fade-in duration-500">
            <div>
                <h1 className="text-3xl font-black tracking-tight">MEIN VERLAUF</h1>
                <p className="text-gray-500 text-sm mt-1">Überblick über alle deine bisherigen Herausforderungen und Ergebnisse.</p>
            </div>

            <div className="flex gap-4 mb-6">
                <button
                    onClick={() => setActiveTab('KASPA')}
                    className={`px-6 py-2 rounded-full font-black text-xs uppercase tracking-widest transition-colors ${activeTab === 'KASPA' ? 'bg-kaspa-primary text-kaspa-dark' : 'bg-kaspa-border text-gray-400 hover:text-white'}`}
                >
                    KaspaBattle Historie
                </button>
                <button
                    onClick={() => setActiveTab('FACEIT')}
                    className={`px-6 py-2 rounded-full font-black text-xs uppercase tracking-widest transition-colors ${activeTab === 'FACEIT' ? 'bg-orange-500 text-white' : 'bg-kaspa-border text-gray-400 hover:text-white'}`}
                >
                    Faceit Historie
                </button>
            </div>

            <div className="card overflow-hidden !p-0">
                <table className="w-full text-left border-collapse">
                    <thead className="bg-kaspa-border/30 text-[10px] font-black uppercase tracking-widest text-gray-500 border-b border-kaspa-border">
                        {activeTab === 'KASPA' ? (
                            <tr>
                                <th className="px-6 py-4">Status</th>
                                <th className="px-6 py-4">Spiel / Modus</th>
                                <th className="px-6 py-4">Gegner</th>
                                <th className="px-6 py-4">Einsatz</th>
                                <th className="px-6 py-4 text-right">Aktion</th>
                            </tr>
                        ) : (
                            <tr>
                                <th className="px-6 py-4">Datum</th>
                                <th className="px-6 py-4">Spiel / Map</th>
                                <th className="px-6 py-4">Modus</th>
                                <th className="px-6 py-4">Ergebnis</th>
                                <th className="px-6 py-4 text-right">Faceit ID</th>
                            </tr>
                        )}
                    </thead>
                    <tbody className="divide-y divide-kaspa-border">
                        {loading ? (
                            <tr>
                                <td colSpan={5} className="px-6 py-20 text-center text-gray-500 italic">Lade Verlauf...</td>
                            </tr>
                        ) : activeTab === 'KASPA' ? (
                            matches.length > 0 ? matches.map(match => {
                                const game = SUPPORTED_GAMES.find(g => g.id === match.game_id);
                                return (
                                    <tr key={match.id} className="hover:bg-kaspa-primary/5 transition-colors group">
                                        <td className="px-6 py-4"><MatchStatusBadge status={match.status} /></td>
                                        <td className="px-6 py-4">
                                            <span className="font-bold text-sm block">{game?.name}</span>
                                            <span className="text-[10px] text-gray-500 uppercase font-black">{match.match_mode}</span>
                                        </td>
                                        <td className="px-6 py-4">
                                            <span className="text-sm font-medium">{match.player_b_faceit_nickname || '---'}</span>
                                        </td>
                                        <td className="px-6 py-4">
                                            <span className="text-kaspa-primary font-black">{formatKas(match.wager_amount_sompi, 0)} KAS</span>
                                        </td>
                                        <td className="px-6 py-4 text-right">
                                            <Link to={`/match/${match.id}`} className="text-xs font-bold text-gray-400 group-hover:text-kaspa-primary group-hover:underline">
                                                DETAILS ↗
                                            </Link>
                                        </td>
                                    </tr>
                                );
                            }) : (
                                <tr>
                                    <td colSpan={5} className="px-6 py-20 text-center text-gray-500 italic">Noch keine Matches gespielt.</td>
                                </tr>
                            )
                        ) : (
                            faceitMatches.length > 0 ? faceitMatches.map(match => {
                                const date = new Date(match.started_at * 1000).toLocaleString();
                                // Parse score if available
                                const scoreObj = match.results?.score;
                                let scoreStr = "N/A";
                                if (scoreObj) {
                                    const scores = Object.values(scoreObj);
                                    if (scores.length >= 2) {
                                        scoreStr = `${scores[0]}:${scores[1]}`;
                                    }
                                }

                                return (
                                    <tr key={match.match_id} className="hover:bg-orange-500/5 transition-colors group">
                                        <td className="px-6 py-4">
                                            <span className="text-gray-400 text-xs">{date}</span>
                                        </td>
                                        <td className="px-6 py-4">
                                            <span className="font-bold text-sm block">{match.game_id}</span>
                                            <span className="text-[10px] text-gray-500 uppercase font-black">{match.map_i_ds?.[0] || 'Unknown Map'}</span>
                                        </td>
                                        <td className="px-6 py-4">
                                            <span className="text-sm font-medium">{match.game_mode}</span>
                                        </td>
                                        <td className="px-6 py-4">
                                            <span className="text-white font-black">{scoreStr}</span>
                                        </td>
                                        <td className="px-6 py-4 text-right">
                                            <a href={`https://www.faceit.com/en/${match.game_id}/room/${match.match_id}`} target="_blank" rel="noreferrer" className="text-xs font-bold text-gray-400 group-hover:text-orange-400 group-hover:underline">
                                                MATCH ROOM ↗
                                            </a>
                                        </td>
                                    </tr>
                                );
                            }) : (
                                <tr>
                                    <td colSpan={5} className="px-6 py-20 text-center text-gray-500 italic">Keine aktuellen FACEIT Matches gefunden.</td>
                                </tr>
                            )
                        )}
                    </tbody>
                </table>
            </div>
        </div>
    );
}
