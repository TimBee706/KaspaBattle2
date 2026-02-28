import { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { SUPPORTED_GAMES, FEE_WINNER_PERCENT } from '../../config/constants';
import { validateWagerAmount } from '../../utils/validation';
import { kasToSompi } from '../../utils/format';
import { createMatch } from '../../api/matches';
import { useWalletStore } from '../../stores/useWalletStore';

export function CreateChallengeForm() {
    const navigate = useNavigate();
    const { isConnected } = useWalletStore();
    const [gameId, setGameId] = useState(SUPPORTED_GAMES[0].id);
    const [mode, setMode] = useState<'bo1' | 'bo3'>('bo1');
    const [wager, setWager] = useState<number | string>(10);
    const [isSubmitting, setIsSubmitting] = useState(false);
    const [error, setError] = useState<string | null>(null);

    // Using parseLocalFloat helper here to calculate potentialWin properly despite comma string inputs
    const wagerNumber = typeof wager === 'string' ? parseFloat(wager.replace(',', '.')) || 0 : wager;
    const validation = validateWagerAmount(wagerNumber);
    const potentialWin = wagerNumber * 2 * (FEE_WINNER_PERCENT / 100);

    const handleSubmit = async (e: React.FormEvent) => {
        e.preventDefault();
        if (!validation.valid || !isConnected) return;

        setIsSubmitting(true);
        setError(null);

        try {
            const response = await createMatch({
                game_id: gameId,
                match_mode: mode,
                wager_amount_sompi: kasToSompi(wager),
            });

            navigate(`/match/${response.match.id}`);
        } catch (err: any) {
            setError(err.response?.data || err.message);
        } finally {
            setIsSubmitting(false);
        }
    };

    return (
        <div className="card max-w-lg mx-auto">
            <h2 className="text-xl font-bold mb-6 flex items-center gap-2">
                <span className="text-kaspa-primary">🏆</span>
                Neue Challenge erstellen
            </h2>

            <form onSubmit={handleSubmit} className="space-y-6">
                {/* Spiel-Auswahl */}
                <div>
                    <label className="block text-xs uppercase tracking-widest text-gray-500 font-bold mb-2">Spiel wählen</label>
                    <div className="grid grid-cols-3 gap-3">
                        {SUPPORTED_GAMES.map((game) => (
                            <button
                                key={game.id}
                                type="button"
                                onClick={() => setGameId(game.id as any)}
                                className={`flex flex-col items-center justify-center p-3 rounded-xl border-2 transition-all ${gameId === game.id
                                    ? 'border-kaspa-primary bg-kaspa-primary/10 text-white'
                                    : 'border-kaspa-border bg-kaspa-dark/50 text-gray-400 hover:border-gray-600'
                                    }`}
                            >
                                <span className="text-2xl mb-1">{game.icon}</span>
                                <span className="text-[10px] font-bold">{game.name}</span>
                            </button>
                        ))}
                    </div>
                </div>

                {/* Modus & Einsatz */}
                <div className="grid grid-cols-2 gap-4">
                    <div>
                        <label className="block text-xs uppercase tracking-widest text-gray-500 font-bold mb-2">Modus</label>
                        <select
                            value={mode}
                            onChange={(e) => setMode(e.target.value as any)}
                            className="w-full bg-kaspa-dark border border-kaspa-border rounded-lg px-3 py-2 text-sm focus:border-kaspa-primary outline-none"
                        >
                            <option value="bo1">Best of 1</option>
                            <option value="bo3">Best of 3</option>
                        </select>
                    </div>
                    <div>
                        <label className="block text-xs uppercase tracking-widest text-gray-500 font-bold mb-2">Einsatz (KAS)</label>
                        <input
                            type="text"
                            inputMode="decimal"
                            value={wager}
                            onChange={(e) => setWager(e.target.value)}
                            className={`w-full bg-kaspa-dark border rounded-lg px-3 py-2 text-sm focus:border-kaspa-primary outline-none ${!validation.valid ? 'border-red-500' : 'border-kaspa-border'
                                }`}
                            placeholder="Min 10 KAS"
                        />
                    </div>
                </div>

                {/* Gewinn-Vorschau */}
                <div className="bg-kaspa-primary/5 border border-kaspa-primary/20 rounded-xl p-4">
                    <div className="flex justify-between items-center mb-1">
                        <span className="text-xs text-gray-400">Dein Einsatz:</span>
                        <span className="text-sm font-bold text-white">{wager} KAS</span>
                    </div>
                    <div className="flex justify-between items-center mb-3">
                        <span className="text-xs text-gray-400">Pot-Gebühren (5%):</span>
                        <span className="text-sm font-bold text-red-400">-{wagerNumber * 0.1} KAS</span>
                    </div>
                    <div className="h-px bg-kaspa-primary/20 mb-3" />
                    <div className="flex justify-between items-center">
                        <span className="text-sm font-bold text-kaspa-primary">Möglicher Gewinn:</span>
                        <span className="text-xl font-black text-kaspa-primary">{potentialWin.toFixed(2)} KAS</span>
                    </div>
                </div>

                {!isConnected && (
                    <p className="text-center text-xs text-orange-400 font-bold">⚠️ Bitte verbinde dein Wallet, um fortzufahren.</p>
                )}

                {error && (
                    <p className="text-center text-xs text-red-500 font-bold">❌ {error}</p>
                )}

                <button
                    type="submit"
                    disabled={!validation.valid || isSubmitting || !isConnected}
                    className="w-full btn-primary h-12 relative overflow-hidden group"
                >
                    {isSubmitting ? '...' : (
                        <>
                            <span className="relative z-10">CHALLENGE VERÖFFENTLICHEN</span>
                            <div className="absolute inset-0 bg-white/20 translate-x-[-100%] group-hover:translate-x-[100%] transition-transform duration-1000" />
                        </>
                    )}
                </button>
            </form>
        </div>
    );
}
