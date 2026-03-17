import { Link } from 'react-router-dom';
import type { BattleMatch } from '../../api/types';
import { formatKas } from '../../utils/format';
import { SUPPORTED_GAMES } from '../../config/constants';
import { MatchStatusBadge } from './MatchStatusBadge';

export function ChallengeCard({ match }: { match: BattleMatch }) {
    const game = SUPPORTED_GAMES.find(g => g.id === match.game_id);
    const createdAt = new Date(match.created_at).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });

    return (
        <div className="card hover:border-kaspa-primary/50 transition-all group relative overflow-hidden">
            <div className="absolute top-0 right-0 p-3 opacity-10 group-hover:opacity-20 transition-opacity">
                {game?.icon && <img src={game.icon} alt={game.name} className="w-16 h-16 object-contain" />}
            </div>

            <div className="flex justify-between items-start mb-4">
                <div>
                    <h3 className="font-bold text-lg leading-tight">{match.player_a_faceit_nickname}</h3>
                    <p className="text-[10px] text-gray-500 uppercase tracking-widest font-bold">{game?.name} • {match.match_mode}</p>
                </div>
                <MatchStatusBadge status={match.status} />
            </div>

            <div className="flex items-end justify-between">
                <div>
                    <span className="text-gray-500 text-[10px] uppercase font-bold block mb-1">Einsatz</span>
                    <span className="text-2xl font-black text-kaspa-primary leading-none">
                        {formatKas(match.wager_amount_sompi, 0)} <span className="text-xs uppercase">KAS</span>
                    </span>
                </div>

                <Link
                    to={`/match/${match.id}`}
                    className="px-4 py-2 bg-kaspa-border hover:bg-kaspa-primary hover:text-kaspa-dark font-bold rounded-lg text-xs transition-all"
                >
                    {match.status === 'OPEN' ? 'ANSEHEN' : 'LIVE'}
                </Link>
            </div>

            <div className="mt-4 pt-3 border-t border-kaspa-border flex justify-between items-center">
                <span className="text-[10px] text-gray-500 bg-kaspa-dark px-2 py-0.5 rounded border border-kaspa-border">
                    #{match.id.slice(0, 8)}
                </span>
                <span className="text-[10px] text-gray-400">
                    Erstellt: {createdAt} Uhr
                </span>
            </div>
        </div>
    );
}
