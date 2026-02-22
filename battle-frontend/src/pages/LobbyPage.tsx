import { useEffect } from 'react';
import { ChallengeCard } from '../components/match/ChallengeCard';
import { useLobbyStore } from '../stores/useLobbyStore';
import { getOpenMatches } from '../api/matches';
import { LOBBY_POLL_INTERVAL_MS, SUPPORTED_GAMES } from '../config/constants';

export function LobbyPage() {
    const { challenges, filters, setChallenges, setLoading, setError, isLoading, setFilters } = useLobbyStore();

    const fetchChallenges = async () => {
        setLoading(true);
        try {
            const data = await getOpenMatches(filters);
            setChallenges(data.matches, data.total);
        } catch (err: any) {
            setError(err.message);
        }
    };

    useEffect(() => {
        fetchChallenges();
        const interval = setInterval(fetchChallenges, LOBBY_POLL_INTERVAL_MS);
        return () => clearInterval(interval);
    }, [filters]);

    return (
        <div className="space-y-8 animate-in fade-in duration-500">
            <div className="flex flex-col md:flex-row justify-between items-start md:items-center gap-4">
                <div>
                    <h1 className="text-3xl font-black tracking-tight">MATCHMAKING LOBBY</h1>
                    <p className="text-gray-500 text-sm mt-1">Hier findest du alle offenen Herausforderungen auf der Kaspa-Blockchain.</p>
                </div>
                <button
                    onClick={() => fetchChallenges()}
                    className="p-2 bg-kaspa-card border border-kaspa-border rounded-lg hover:bg-kaspa-border transition-colors text-kaspa-primary"
                    title="Aktualisieren"
                >
                    🔄
                </button>
            </div>

            {/* Filter Bar */}
            <div className="flex flex-wrap gap-3 py-4 border-y border-kaspa-border">
                <button
                    onClick={() => setFilters({ game_id: undefined })}
                    className={`px-4 py-1.5 rounded-full text-xs font-bold transition-all ${!filters.game_id ? 'bg-kaspa-primary text-kaspa-dark' : 'bg-kaspa-card border border-kaspa-border text-gray-400 hover:text-white'}`}
                >
                    ALLE SPIELE
                </button>
                {SUPPORTED_GAMES.map(game => (
                    <button
                        key={game.id}
                        onClick={() => setFilters({ game_id: game.id })}
                        className={`px-4 py-1.5 rounded-full text-xs font-bold transition-all ${filters.game_id === game.id ? 'bg-kaspa-primary text-kaspa-dark' : 'bg-kaspa-card border border-kaspa-border text-gray-400 hover:text-white'}`}
                    >
                        {game.name.toUpperCase()}
                    </button>
                ))}

                <div className="hidden md:block w-px h-6 bg-kaspa-border mx-2" />

                <select
                    className="bg-kaspa-card border border-kaspa-border rounded-full px-4 py-1.5 text-xs font-bold text-gray-400 outline-none focus:border-kaspa-primary"
                    onChange={(e) => setFilters({ match_mode: e.target.value || undefined })}
                >
                    <option value="">ALLE MODI</option>
                    <option value="bo1">BEST OF 1</option>
                    <option value="bo3">BEST OF 3</option>
                </select>
            </div>

            {/* Challenge List */}
            {isLoading && challenges.length === 0 ? (
                <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6">
                    {[1, 2, 3].map(i => (
                        <div key={i} className="card h-48 animate-pulse bg-kaspa-border/20" />
                    ))}
                </div>
            ) : challenges.length > 0 ? (
                <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6">
                    {challenges.map((match) => (
                        <ChallengeCard key={match.id} match={match} />
                    ))}
                </div>
            ) : (
                <div className="py-20 text-center bg-kaspa-card/30 rounded-3xl border border-dashed border-kaspa-border">
                    <div className="text-5xl mb-4 opacity-30">🏜️</div>
                    <h3 className="text-xl font-bold text-gray-400">Keine Challenges gefunden</h3>
                    <p className="text-gray-500 text-sm mt-2">Sei der Erste und erstelle eine eigene Herausforderung!</p>
                    <a href="/create" className="btn-primary inline-block mt-6 px-8 py-3">CHALLENGE ERSTELLEN</a>
                </div>
            )}
        </div>
    );
}
