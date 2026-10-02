import { useCallback, useEffect, useState } from 'react';
import { Link, useNavigate } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { createFreePlayGame, joinFreePlayGame, listFreePlayLobbies } from '../api/freePlay';
import type { FpActiveItem, FpDifficulty, FpLobbyItem } from '../domain/freePlay';
import { useAuthStore } from '../stores/useAuthStore';
import { PageHeader } from '../components/common/PageHeader';
import { TestnetComingSoon } from '../components/common/TestnetComingSoon';
import { FormAlert } from '../components/auth/AuthCard';
import { KaspaCoin } from '../components/native/ConnectFourCell';
import { useGameSocket } from '../hooks/useGameSocket';
import { getApiErrorCode } from '../utils/errors';

export function FreePlayLobbyPage() {
    const { t } = useTranslation();
    const navigate = useNavigate();
    const isAuthenticated = useAuthStore((s) => s.isAuthenticated);
    const isAuthLoading = useAuthStore((s) => s.isAuthLoading);
    const [lobbies, setLobbies] = useState<FpLobbyItem[]>([]);
    const [active, setActive] = useState<FpActiveItem[]>([]);
    const [difficulty, setDifficulty] = useState<FpDifficulty>('easy');
    const [busy, setBusy] = useState<string | null>(null);
    const [error, setError] = useState<string | null>(null);

    const reload = useCallback(async () => {
        try {
            const res = await listFreePlayLobbies();
            setLobbies(res.lobbies);
            setActive(res.active);
        } catch {
            /* transient – the next poll retries */
        }
    }, []);

    useEffect(() => {
        if (!isAuthenticated) return;
        void reload();
        const id = setInterval(() => void reload(), 10_000);
        return () => clearInterval(id);
    }, [isAuthenticated, reload]);

    useGameSocket({
        enabled: isAuthenticated,
        onMessage: (m) => {
            if ((m as { type?: string })?.type === 'free_play_lobby') void reload();
        },
        onOpen: () => void reload(),
    });

    const fail = (err: unknown) => {
        const code = getApiErrorCode(err);
        setError(code && ['too_many_open_lobbies', 'too_many_active_games', 'lobby_not_open', 'own_lobby', 'rate_limited'].includes(code) ? t(`freeplay.errors.${code}`) : t('freeplay.errors.generic'));
    };

    const create = async (kind: 'human' | 'bot') => {
        setBusy(kind);
        setError(null);
        try {
            const g = await createFreePlayGame(kind, difficulty);
            navigate(`/free-play/${g.id}`);
        } catch (err) {
            fail(err);
        } finally {
            setBusy(null);
        }
    };

    const join = async (id: string) => {
        setBusy(id);
        setError(null);
        try {
            const g = await joinFreePlayGame(id);
            navigate(`/free-play/${g.id}`);
        } catch (err) {
            fail(err);
            void reload();
        } finally {
            setBusy(null);
        }
    };

    if (isAuthLoading) {
        return <div className="flex min-h-[40vh] items-center justify-center"><div className="h-8 w-8 animate-spin rounded-full border-2 border-kaspa-primary border-t-transparent" /></div>;
    }

    if (!isAuthenticated) {
        return (
            <div>
                <PageHeader icon="bolt" title={t('freeplay.title')} subtitle={t('freeplay.subtitle')} />
                <div className="glass-panel mx-auto max-w-xl space-y-5 p-8 text-center" data-testid="free-play-signed-out">
                    <div className="flex justify-center gap-1"><KaspaCoin color="blue" className="h-10 w-10" /><KaspaCoin color="red" className="h-10 w-10" /></div>
                    <p className="text-gray-300">{t('freeplay.explainer')}</p>
                    <div className="flex flex-col justify-center gap-3 sm:flex-row">
                        <Link to="/register" className="btn-primary flex h-12 items-center justify-center px-8">{t('freeplay.register_cta')}</Link>
                        <Link to="/login?next=/free-play" className="glass-button flex h-12 items-center justify-center px-8 font-bold">{t('auth.login.submit')}</Link>
                    </div>
                </div>
            </div>
        );
    }

    const mine = lobbies.filter((l) => l.mine);
    const others = lobbies.filter((l) => !l.mine);

    return (
        <div>
            <PageHeader icon="bolt" title={t('freeplay.title')} subtitle={t('freeplay.explainer')} />
            {error && <div className="mb-4"><FormAlert kind="error">{error}</FormAlert></div>}

            <div className="mb-8 grid grid-cols-1 gap-4 md:grid-cols-2">
                <section className="glass-panel space-y-4 p-6" aria-labelledby="fp-pvp">
                    <h2 id="fp-pvp" className="text-lg font-black uppercase tracking-tight text-white">{t('freeplay.vs_player')}</h2>
                    <p className="text-sm text-gray-400">{t('freeplay.vs_player_text')}</p>
                    <button type="button" className="btn-primary h-12 w-full" disabled={busy !== null} onClick={() => void create('human')}>
                        {busy === 'human' ? t('common.loading') : t('freeplay.create_lobby')}
                    </button>
                </section>
                <section className="glass-panel space-y-4 p-6" aria-labelledby="fp-bot">
                    <h2 id="fp-bot" className="text-lg font-black uppercase tracking-tight text-white">{t('freeplay.vs_bot')}</h2>
                    <p className="text-sm text-gray-400">{t('freeplay.vs_bot_text')}</p>
                    <div>
                        <label htmlFor="fp-difficulty" className="form-label">{t('freeplay.difficulty')}</label>
                        <select id="fp-difficulty" className="form-input" value={difficulty} onChange={(e) => setDifficulty(e.target.value as FpDifficulty)}>
                            <option value="easy">{t('freeplay.diff.easy')}</option>
                            <option value="medium">{t('freeplay.diff.medium')}</option>
                            <option value="hard">{t('freeplay.diff.hard')}</option>
                        </select>
                    </div>
                    <button type="button" className="btn-primary h-12 w-full" disabled={busy !== null} onClick={() => void create('bot')}>
                        {busy === 'bot' ? t('common.loading') : t('freeplay.start_bot')}
                    </button>
                </section>
            </div>

            {active.length > 0 && (
                <section className="mb-8" aria-labelledby="fp-active">
                    <h2 id="fp-active" className="mb-3 text-sm font-black uppercase tracking-widest text-gray-500">{t('freeplay.my_games')}</h2>
                    <ul className="space-y-2">
                        {active.map((g) => (
                            <li key={g.id} className="glass-panel flex items-center justify-between gap-3 p-4">
                                <span className="text-sm text-white">
                                    {g.opponentKind === 'bot' ? t('freeplay.bot_label', { level: t(`freeplay.diff.${g.botDifficulty ?? 'easy'}`) }) : g.opponentName}
                                    <span className="ml-2 text-xs text-gray-500">{t('freeplay.moves', { count: g.moveCount })}</span>
                                </span>
                                <Link to={`/free-play/${g.id}`} className="text-sm font-bold text-kaspa-primary hover:underline">
                                    {g.yourTurn ? t('freeplay.your_turn') : t('freeplay.continue')}
                                </Link>
                            </li>
                        ))}
                    </ul>
                </section>
            )}

            <section aria-labelledby="fp-open" className="mb-8">
                <h2 id="fp-open" className="mb-3 text-sm font-black uppercase tracking-widest text-gray-500">{t('freeplay.open_lobbies')}</h2>
                {others.length === 0 && mine.length === 0 ? (
                    <p className="glass-panel p-6 text-center text-sm text-gray-500">{t('freeplay.no_lobbies')}</p>
                ) : (
                    <ul className="space-y-2">
                        {mine.map((l) => (
                            <li key={l.id} className="glass-panel flex items-center justify-between gap-3 border-kaspa-primary/30 p-4">
                                <span className="text-sm text-gray-300">{t('freeplay.waiting_for_opponent')}</span>
                                <Link to={`/free-play/${l.id}`} className="text-sm font-bold text-kaspa-primary hover:underline">{t('freeplay.open')}</Link>
                            </li>
                        ))}
                        {others.map((l) => (
                            <li key={l.id} className="glass-panel flex items-center justify-between gap-3 p-4">
                                <span className="text-sm font-bold text-white">{l.creatorName}</span>
                                <button type="button" className="btn-primary h-10 px-5 text-sm" disabled={busy !== null} onClick={() => void join(l.id)}>
                                    {busy === l.id ? t('common.loading') : t('freeplay.join')}
                                </button>
                            </li>
                        ))}
                    </ul>
                )}
            </section>

            <TestnetComingSoon />
        </div>
    );
}
