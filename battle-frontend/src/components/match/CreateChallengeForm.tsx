import { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { SUPPORTED_GAMES, NATIVE_GAMES, FEE_WINNER_PERCENT, FEE_TREASURY_PERCENT } from '../../config/constants';
import { validateWagerAmount } from '../../utils/validation';
import { useWalletStore } from '../../stores/useWalletStore';
import { useAuthStore } from '../../stores/useAuthStore';
import { useLobby } from '../../hooks/useLobby';
import { useTranslation } from 'react-i18next';
import type { GameId } from '../../config/constants';
import type { MatchMode, MatchProvider } from '../../api/types';
import { useAccess } from '../../hooks/useAccess';
import { canPlayProvider } from '../../domain/access';
import { KaspaCoin } from '../native/ConnectFourCell';
import { getErrorMessage } from '../../utils/errors';
import { FEATURE_FLAGS } from '../../config/featureFlags';
import { Icon } from '../Icon';
import { FormField } from '../common/FormField';

export function CreateChallengeForm({ provider = 'FACEIT' }: { provider?: MatchProvider }) {
    const navigate = useNavigate();
    const { isConnected } = useWalletStore();
    const { isAuthLoading } = useAuthStore();
    const access = useAccess();
    const isNative = provider === 'NATIVE';
    const { createChallenge, isCreating } = useLobby();

    const [gameId, setGameId] = useState<string>(isNative ? NATIVE_GAMES[0].id : SUPPORTED_GAMES[0].id);
    const [mode, setMode] = useState<'BO1' | 'BO3'>('BO1');
    const [wager, setWager] = useState<number | string>(10);
    const [error, setError] = useState<string | null>(null);
    const { t } = useTranslation();

    const wagerNumber = typeof wager === 'string' ? parseFloat(wager.replace(',', '.')) || 0 : wager;
    const validation = validateWagerAmount(wagerNumber);
    const potentialWin = wagerNumber * 2 * (FEE_WINNER_PERCENT / 100);
    const feeAmount = wagerNumber * 2 * (FEE_TREASURY_PERCENT / 100);
    const wagerTouched = wagerNumber > 0;

    const handleSubmit = async (e: React.FormEvent) => {
        e.preventDefault();

        // Solange der Auth-State noch geladen wird, zeigen wir keinen harten Fehler,
        // sondern deaktivieren nur den Button.
        if (isAuthLoading) {
            return;
        }

        // Browser games need login + wallet only; FACEIT games also need a linked FACEIT account
        // (the server enforces the same rule).
        const hasRequiredAuth = canPlayProvider(access, provider) || (FEATURE_FLAGS.TEST_MODE && isNative && isConnected);
        if (!validation.valid || !hasRequiredAuth) {
            // Keine weiteren Seiteneffekte – die UI zeigt Hinweise unterhalb des Formulars.
            return;
        }

        setError(null);

        try {
            const result = await createChallenge({
                stakeKas: wagerNumber,
                mode: isNative ? 'BO1' : mode,
                gameId,
            });

            if (result && result.id) {
                navigate(`/escrow/${result.id}`);
            }
        } catch (err) {
            setError(getErrorMessage(err, t('common.error', { message: '' })));
        }
    };

    const canCreate = canPlayProvider(access, provider) || (FEATURE_FLAGS.TEST_MODE && isNative && isConnected);
    const needsFaceitFirst = !isNative && !canCreate && isConnected;

    return (
        <div className="glass-panel p-8">
            <form onSubmit={handleSubmit} className="space-y-7">
                {/* Spiel-Auswahl */}
                <FormField label={t('challenge.select_game')}>
                    <div className="grid grid-cols-2 sm:grid-cols-3 gap-3">
                        {isNative && NATIVE_GAMES.map((game) => (
                            <button
                                key={game.id}
                                type="button"
                                onClick={() => setGameId(game.id)}
                                aria-pressed={gameId === game.id}
                                className="flex flex-col items-center justify-center p-3 rounded-xl border transition-all border-kaspa-primary bg-kaspa-primary/10 text-white shadow-glow-subtle"
                            >
                                <span className="mb-1 flex gap-0.5">
                                    <KaspaCoin color="blue" className="h-6 w-6" />
                                    <KaspaCoin color="red" className="h-6 w-6" />
                                </span>
                                <span className="text-2xs font-bold">{t(game.nameKey)}</span>
                                <span className="mt-1 text-[9px] font-semibold text-gray-400 text-center leading-tight">
                                    {t('native.create.players')} · {t('native.create.in_browser')}
                                </span>
                            </button>
                        ))}
                        {!isNative && SUPPORTED_GAMES.map((game) => (
                            <button
                                key={game.id}
                                type="button"
                                onClick={() => setGameId(game.id as GameId)}
                                aria-pressed={gameId === game.id}
                                className={`flex flex-col items-center justify-center p-3 rounded-xl border transition-all ${gameId === game.id
                                    ? 'border-kaspa-primary bg-kaspa-primary/10 text-white shadow-glow-subtle'
                                    : 'border-kaspa-border bg-kaspa-dark/50 text-gray-400 hover:border-gray-600'
                                    }`}
                            >
                                {game.icon ? (
                                    <img src={game.icon} alt={game.name} className="w-8 h-8 mb-1 object-contain" />
                                ) : (
                                    <Icon name="list" className="w-6 h-6 text-kaspa-primary/50" />
                                )}
                                <span className="text-2xs font-bold">{game.name}</span>
                            </button>
                        ))}
                    </div>
                </FormField>

                {/* Modus & Einsatz */}
                <div className="grid grid-cols-1 sm:grid-cols-2 gap-5">
                    {!isNative && <FormField label={t('challenge.mode')} htmlFor="challenge-mode">
                        <select
                            id="challenge-mode"
                            value={mode}
                            onChange={(e) => setMode(e.target.value as MatchMode)}
                            className="form-input"
                        >
                            <option value="BO1">Best of 1</option>
                            <option value="BO3">Best of 3</option>
                        </select>
                    </FormField>}
                    <FormField
                        label={t('challenge.stake')}
                        htmlFor="challenge-stake"
                        error={wagerTouched && !validation.valid ? validation.error : undefined}
                        hint={!wagerTouched || validation.valid ? t('challenge.min_wager') : undefined}
                    >
                        <div className="relative">
                            <input
                                id="challenge-stake"
                                type="text"
                                inputMode="decimal"
                                value={wager}
                                onChange={(e) => setWager(e.target.value)}
                                className={`form-input pr-12 ${wagerTouched && !validation.valid ? 'form-input-error' : ''}`}
                                placeholder={t('challenge.min_wager')}
                            />
                            <span className="absolute right-3 top-1/2 -translate-y-1/2 text-2xs font-bold text-gray-500 uppercase tracking-wider pointer-events-none">
                                KAS
                            </span>
                        </div>
                    </FormField>
                </div>

                {/* Gewinn-Vorschau */}
                <div className="bg-kaspa-primary/5 border border-kaspa-primary/20 rounded-xl p-5">
                    <h3 className="text-2xs font-black text-gray-500 uppercase tracking-widest mb-3">
                        {t('challenge.preview.title')}
                    </h3>
                    <div className="flex justify-between items-center mb-1">
                        <span className="text-xs text-gray-400">{t('challenge.preview.your_wager')}</span>
                        <span className="text-sm font-bold text-white">{wager || 0} KAS</span>
                    </div>
                    <div className="flex justify-between items-center mb-3">
                        <span className="text-xs text-gray-400">
                            {t('challenge.preview.fees', { percent: FEE_TREASURY_PERCENT })}
                        </span>
                        <span className="text-sm font-bold text-red-400">-{feeAmount.toFixed(2)} KAS</span>
                    </div>
                    <div className="h-px bg-kaspa-primary/20 mb-3" />
                    <div className="flex justify-between items-center">
                        <span className="text-sm font-bold text-kaspa-primary">{t('challenge.preview.potential_win')}</span>
                        <span className="text-xl font-black text-kaspa-primary">{potentialWin.toFixed(2)} KAS</span>
                    </div>
                </div>

                {isNative && (
                    <p className="text-center text-xs font-semibold text-kaspa-primary/90" data-testid="native-no-faceit-note">
                        {t('native.create.no_faceit')}
                    </p>
                )}

                {(!isConnected || !canCreate) && (
                    <p className="text-center text-xs text-amber-400 font-bold flex items-center justify-center gap-1.5">
                        <Icon name="alert-triangle" className="w-3.5 h-3.5 shrink-0" />
                        {isAuthLoading
                            ? t('challenge.auth_loading')
                            : needsFaceitFirst
                                ? t('challenge.connect_faceit_hint')
                                : t('challenge.wallet_needed')}
                    </p>
                )}

                {error && (
                    <p className="text-center text-xs text-red-500 font-bold flex items-center justify-center gap-1.5">
                        <Icon name="x" className="w-3.5 h-3.5 shrink-0" /> {error}
                    </p>
                )}

                <button
                    type="submit"
                    disabled={
                        isAuthLoading
                        || !validation.valid
                        || isCreating
                        || !canCreate
                    }
                    title={needsFaceitFirst && !isAuthLoading ? t('challenge.connect_faceit_hint') : undefined}
                    className="w-full btn-primary h-12 relative overflow-hidden group disabled:opacity-40 disabled:cursor-not-allowed"
                >
                    {isCreating
                        ? t('challenge.creating')
                        : isAuthLoading
                            ? t('challenge.auth_loading_button')
                            : (
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
