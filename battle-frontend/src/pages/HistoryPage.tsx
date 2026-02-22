import { useEffect, useState } from 'react';
import { getMyMatches } from '../api/matches';
import type { BattleMatch } from '../api/types';
import { MatchStatusBadge } from '../components/match/MatchStatusBadge';
import { formatKas } from '../utils/format';
import { SUPPORTED_GAMES } from '../config/constants';
import { Link } from 'react-router-dom';

export function HistoryPage() {
    const [matches, setMatches] = useState<BattleMatch[]>([]);
    const [loading, setLoading] = useState(true);

    useEffect(() => {
        getMyMatches()
            .then(res => setMatches(res.matches))
            .finally(() => setLoading(false));
    }, []);

    return (
        <div className="max-w-5xl mx-auto space-y-8 animate-in fade-in duration-500">
            <div>
                <h1 className="text-3xl font-black tracking-tight">MEIN VERLAUF</h1>
                <p className="text-gray-500 text-sm mt-1">Überblick über alle deine bisherigen Herausforderungen und Ergebnisse.</p>
            </div>

            <div className="card overflow-hidden !p-0">
                <table className="w-full text-left border-collapse">
                    <thead className="bg-kaspa-border/30 text-[10px] font-black uppercase tracking-widest text-gray-500 border-b border-kaspa-border">
                        <tr>
                            <th className="px-6 py-4">Status</th>
                            <th className="px-6 py-4">Spiel / Modus</th>
                            <th className="px-6 py-4">Gegner</th>
                            <th className="px-6 py-4">Einsatz</th>
                            <th className="px-6 py-4 text-right">Aktion</th>
                        </tr>
                    </thead>
                    <tbody className="divide-y divide-kaspa-border">
                        {loading ? (
                            <tr>
                                <td colSpan={5} className="px-6 py-20 text-center text-gray-500 italic">Lade Verlauf...</td>
                            </tr>
                        ) : matches.length > 0 ? matches.map(match => {
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
                        )}
                    </tbody>
                </table>
            </div>
        </div>
    );
}
