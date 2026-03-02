import { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { SUPPORTED_GAMES, FEE_WINNER_PERCENT } from '../../config/constants';
import { validateWagerAmount } from '../../utils/validation';
import { useWalletStore } from '../../stores/useWalletStore';
import { useLobby } from '../../hooks/useLobby';
import { useTranslation } from 'react-i18next';

export function CreateChallengeForm() {
    const navigate = useNavigate();
    const { isConnected } = useWalletStore();
    const { createChallenge, isCreating } = useLobby();

    const [gameId, setGameId] = useState(SUPPORTED_GAMES[0].id);
    const [mode, setMode] = useState<'BO1' | 'BO3'>('BO1');
    const [wager, setWager] = useState<number | string>(10);
    const [error, setError] = useState<string | null>(null);
    const { t } = useTranslation();

    const wagerNumber = typeof wager === 'string' ? parseFloat(wager.replace(',', '.')) || 0 : wager;
    const validation = validateWagerAmount(wagerNumber);
    const potentialWin = wagerNumber * 2 * (FEE_WINNER_PERCENT / 100);

    const handleSubmit = async (e: React.FormEvent) => {
        e.preventDefault();
        if (!validation.valid || !isConnected) return;

        setError(null);

        try {
            const result = await createChallenge({
                stakeKas: wagerNumber,
                mode: mode as any,
            });

            if (result && result.id) {
                navigate(`/match/${result.id}`);
            }
        } catch (err: any) {
            setError(err.message);
        }
    };

    return (
        <div className="card max-w-lg mx-auto">
            <h2 className="text-xl font-bold mb-6 flex items-center gap-2">
                <span className="text-kaspa-primary">🏆</span>
                {t('challenge.create_title')}
            </h2>

            <form onSubmit={handleSubmit} className="space-y-6">
                {/* Spiel-Auswahl */}
                <div>
                    <label className="block text-xs uppercase tracking-widest text-gray-500 font-bold mb-2">{t('challenge.select_game')}</label>
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
                        <label className="block text-xs uppercase tracking-widest text-gray-500 font-bold mb-2">{t('challenge.mode')}</label>
                        <select
                            value={mode}
                            onChange={(e) => setMode(e.target.value as any)}
                            className="w-full bg-kaspa-dark border border-kaspa-border rounded-lg px-3 py-2 text-sm focus:border-kaspa-primary outline-none"
                        >
                            <option value="BO1">Best of 1</option>
                            <option value="BO3">Best of 3</option>
                        </select>
                    </div>
                    <div>
                        <label className="block text-xs uppercase tracking-widest text-gray-500 font-bold mb-2">{t('challenge.stake')}</label>
                        <input
                            type="text"
                            inputMode="decimal"
                            value={wager}
                            onChange={(e) => setWager(e.target.value)}
                            className={`w-full bg-kaspa-dark border rounded-lg px-3 py-2 text-sm focus:border-kaspa-primary outline-none ${!validation.valid ? 'border-red-500' : 'border-kaspa-border'
                                }`}
                            placeholder={t('challenge.min_wager')}
                        />
                    </div>
                </div>

                {/* Gewinn-Vorschau */}
                <div className="bg-kaspa-primary/5 border border-kaspa-primary/20 rounded-xl p-4">
                    <div className="flex justify-between items-center mb-1">
                        <span className="text-xs text-gray-400">{t('challenge.preview.your_wager')}</span>
                        <span className="text-sm font-bold text-white">{wager} KAS</span>
                    </div>
                    <div className="flex justify-between items-center mb-3">
                        <span className="text-xs text-gray-400">{t('challenge.preview.fees')}</span>
                        <span className="text-sm font-bold text-red-400">-{wagerNumber * 0.1} KAS</span>
                    </div>
                    <div className="h-px bg-kaspa-primary/20 mb-3" />
                    <div className="flex justify-between items-center">
                        <span className="text-sm font-bold text-kaspa-primary">{t('challenge.preview.potential_win')}</span>
                        <span className="text-xl font-black text-kaspa-primary">{potentialWin.toFixed(2)} KAS</span>
                    </div>
                </div>

                {!isConnected && (
                    <p className="text-center text-xs text-orange-400 font-bold">⚠️ {t('challenge.wallet_needed')}</p>
                )}

                {error && (
                    <p className="text-center text-xs text-red-500 font-bold">❌ {error}</p>
                )}

                <button
                    type="submit"
                    disabled={!validation.valid || isCreating || !isConnected}
                    className="w-full btn-primary h-12 relative overflow-hidden group"
                >
                    {isCreating ? t('challenge.creating', 'Erstelle...') : (
                        <>
                            <span className="relative z-10">{t('challenge.publish')}</span>
                            <div className="absolute inset-0 bg-white/20 translate-x-[-100%] group-hover:translate-x-[100%] transition-transform duration-1000" />
                        </>
                    )}
                </button>
            </form>
        </div>
    );
}
