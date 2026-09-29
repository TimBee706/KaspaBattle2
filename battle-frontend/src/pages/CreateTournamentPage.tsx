import { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { createTournament } from '../api/tournaments';
import { Icon } from '../components/Icon';
import { useWalletStore } from '../stores/useWalletStore';
import { SUPPORTED_GAMES } from '../config/constants';


export function CreateTournamentPage() {
    const { t } = useTranslation();
    const navigate = useNavigate();
    const { isConnected } = useWalletStore();

    const cs2Game = SUPPORTED_GAMES.find(g => g.id === 'cs2') || SUPPORTED_GAMES[0];


    const [form, setForm] = useState({
        name: '',
        game_id: 'cs2',
        max_teams: 8,
        buy_in_sompi: 500_000_000, // 5 KAS
        prize_winner_pct: 70,
        prize_runner_up_pct: 20,
        platform_fee_pct: 10,
        registration_deadline: '',
    });
    const [loading, setLoading] = useState(false);
    const [error, setError] = useState('');

    const handleSubmit = async (e: React.FormEvent) => {
        e.preventDefault();
        setError('');
        setLoading(true);
        try {
            const payload = {
                ...form,
                registration_deadline: form.registration_deadline
                    ? new Date(form.registration_deadline).toISOString()
                    : undefined,
            };
            const t = await createTournament(payload);
            navigate(`/tournaments/${t.id}`);
        } catch (err: unknown) {
            const msg = (err as { response?: { data?: { message?: string } } })?.response?.data?.message ?? t('tournaments.create.error_fallback');
            setError(msg);
        } finally {
            setLoading(false);
        }
    };

    const pct = form.prize_winner_pct + form.prize_runner_up_pct + form.platform_fee_pct;

    return (
        <div>
            {/* Header */}
            <div className="flex flex-col md:flex-row md:justify-between items-start md:items-center gap-6 mb-8">
                <div>
                    <h1 className="text-4xl font-black text-white uppercase tracking-tighter mb-2">{t('tournaments.create.title')}</h1>
                    <p className="text-slate-500 text-sm font-bold uppercase tracking-widest">{t('tournaments.create.game_hint')}</p>
                </div>
            </div>

            <div className="max-w-lg mx-auto">
                <div className="glass-panel p-8">
                    <form onSubmit={handleSubmit} className="space-y-6">
                        {/* Name Input */}
                        <div>
                            <label className="block text-xs uppercase tracking-widest text-gray-500 font-bold mb-2">{t('tournaments.create.name')}</label>
                            <input
                                id="create-tournament-name"
                                className="w-full bg-kaspa-dark border border-kaspa-border rounded-lg px-3 py-2 text-sm text-white focus:outline-none focus:border-kaspa-primary transition-colors"
                                required minLength={3}
                                value={form.name}
                                onChange={e => setForm(f => ({ ...f, name: e.target.value }))}
                                placeholder={t('tournaments.create.name_placeholder')}
                            />
                        </div>
                        
                        {/* Game Selection (Matching Lobby Style) */}
                        <div>
                            <label className="block text-xs uppercase tracking-widest text-gray-500 font-bold mb-2">{t('tournaments.create.game_label')}</label>
                            <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
                                <button
                                    type="button"
                                    className="flex flex-col items-center justify-center p-3 rounded-xl border-2 border-kaspa-primary bg-kaspa-primary/10 text-white transition-all shadow-glow-subtle"
                                >
                                    <img src={cs2Game.icon} alt={cs2Game.name} className="w-8 h-8 mb-1 object-contain" />
                                    <span className="text-[10px] font-bold">{cs2Game.name}</span>
                                </button>
                            </div>
                        </div>

                        {/* Teams & Buy-In */}
                        <div className="grid grid-cols-2 gap-4">
                            <div>
                                <label className="block text-xs uppercase tracking-widest text-gray-500 font-bold mb-2">{t('tournaments.create.max_teams')}</label>
                                <select
                                    id="create-tournament-max-teams"
                                    className="w-full bg-kaspa-dark border border-kaspa-border rounded-lg px-3 py-2 text-sm text-white focus:outline-none focus:border-kaspa-primary"
                                    value={form.max_teams}
                                    onChange={e => setForm(f => ({ ...f, max_teams: Number(e.target.value) }))}
                                >
                                    {[4, 8, 16].map(n => <option key={n} value={n}>{t('tournaments.create.teams_count', { count: n })}</option>)}
                                </select>
                            </div>
                            <div>
                                <label className="block text-xs uppercase tracking-widest text-gray-500 font-bold mb-2">{t('tournaments.create.buy_in')}</label>
                                <div className="relative">
                                    <input
                                        id="create-tournament-buyin"
                                        type="number" min={0} step={0.01}
                                        className="w-full bg-kaspa-dark border border-kaspa-border rounded-lg px-3 py-2 pr-10 text-sm text-white focus:outline-none focus:border-kaspa-primary"
                                        value={form.buy_in_sompi / 1e8}
                                        onChange={e => setForm(f => ({ ...f, buy_in_sompi: Math.round(Number(e.target.value) * 1e8) }))}
                                    />
                                    <span className="absolute right-3 top-1/2 -translate-y-1/2 text-[10px] font-bold text-kaspa-primary">KAS</span>
                                </div>
                            </div>
                        </div>

                        {/* Prize Pool Preview (Matching Lobby Style) */}
                        <div className="bg-kaspa-primary/5 border border-kaspa-primary/20 rounded-xl p-4 space-y-3">
                            <div className="flex justify-between items-center pb-2 border-b border-kaspa-primary/10">
                                <span className="text-xs text-gray-400 uppercase tracking-widest font-bold">{t('tournaments.card.prize_pool')}</span>
                                <span className="text-lg font-black text-white">{(form.buy_in_sompi * form.max_teams / 1e8).toFixed(2)} KAS</span>
                            </div>
                            
                            <div className="space-y-2">
                                <div className="flex justify-between items-center">
                                    <span className="text-[10px] text-gray-500 uppercase font-bold">{t('tournaments.detail.prize_banner.winner')} ({form.prize_winner_pct}%)</span>
                                    <span className="text-xs font-bold text-emerald-400">{(form.buy_in_sompi * form.max_teams * form.prize_winner_pct / 100 / 1e8).toFixed(2)} KAS</span>
                                </div>
                                <div className="flex justify-between items-center">
                                    <span className="text-[10px] text-gray-500 uppercase font-bold">{t('tournaments.detail.prize_banner.runner_up')} ({form.prize_runner_up_pct}%)</span>
                                    <span className="text-xs font-bold text-gray-300">{(form.buy_in_sompi * form.max_teams * form.prize_runner_up_pct / 100 / 1e8).toFixed(2)} KAS</span>
                                </div>
                                <div className="flex justify-between items-center">
                                    <span className="text-[10px] text-gray-500 uppercase font-bold">{t('tournaments.detail.prize_banner.fee')} ({form.platform_fee_pct}%)</span>
                                    <span className="text-xs font-bold text-red-400">-{ (form.buy_in_sompi * form.max_teams * form.platform_fee_pct / 100 / 1e8).toFixed(2) } KAS</span>
                                </div>
                            </div>

                            {pct !== 100 && (
                                <p className="text-orange-400 text-[10px] font-bold flex items-center gap-1 pt-1 border-t border-kaspa-primary/10">
                                    <Icon name="alert-triangle" className="w-3.5 h-3.5" />
                                    {t('tournaments.create.pct_warning', { pct })}
                                </p>
                            )}
                        </div>

                        {/* Prize Split Configuration (Simplified) */}
                        <div>
                            <label className="block text-xs uppercase tracking-widest text-gray-500 font-bold mb-2">{t('tournaments.create.prize_split')}</label>
                            <div className="grid grid-cols-3 gap-2">
                                {(['prize_winner_pct', 'prize_runner_up_pct', 'platform_fee_pct'] as const).map((key) => (
                                    <div key={key}>
                                        <div className="relative">
                                            <input
                                                type="number" min={0} max={100}
                                                className="w-full bg-kaspa-dark border border-kaspa-border rounded-lg px-3 py-2 pr-7 text-xs text-white focus:outline-none focus:border-kaspa-primary"
                                                value={form[key]}
                                                onChange={e => setForm(f => ({ ...f, [key]: Number(e.target.value) }))}
                                            />
                                            <span className="absolute right-2 top-1/2 -translate-y-1/2 text-[10px] text-gray-500">%</span>
                                        </div>
                                    </div>
                                ))}
                            </div>
                        </div>

                        <div>
                            <label className="block text-xs uppercase tracking-widest text-gray-500 font-bold mb-2">{t('tournaments.create.deadline')}</label>
                            <input
                                type="datetime-local"
                                className="w-full bg-kaspa-dark border border-kaspa-border rounded-lg px-3 py-2 text-sm text-white focus:outline-none focus:border-kaspa-primary"
                                value={form.registration_deadline}
                                onChange={e => setForm(f => ({ ...f, registration_deadline: e.target.value }))}
                            />
                        </div>

                        {(!isConnected) && (
                            <p className="text-center text-xs text-orange-400 font-bold flex items-center justify-center gap-1.5">
                                <Icon name="alert-triangle" className="w-3.5 h-3.5 shrink-0" />
                                {t('challenge.wallet_needed')}
                            </p>
                        )}

                        {error && (
                            <p className="text-center text-xs text-red-500 font-bold flex items-center justify-center gap-1.5">
                                <Icon name="x" className="w-3.5 h-3.5 shrink-0" /> {error}
                            </p>
                        )}

                        <button
                            id="create-tournament-submit"
                            type="submit"
                            disabled={loading || pct !== 100 || !form.name || !isConnected}
                            className="w-full btn-primary h-12 relative overflow-hidden group disabled:opacity-50 disabled:cursor-not-allowed"
                        >
                            {loading ? t('tournaments.create.creating') : (
                                <>
                                    <span className="relative z-10">{t('tournaments.create.submit')}</span>
                                    <div className="absolute inset-0 bg-white/20 translate-x-[-100%] group-hover:translate-x-[100%] transition-transform duration-1000" />
                                </>
                            )}
                        </button>
                    </form>
                </div>

                <div className="mt-6 grid grid-cols-2 gap-4">
                    <div className="bg-slate-900/60 border border-slate-700/50 rounded-2xl p-4 flex flex-col items-center hover:-translate-y-0.5 transition-transform">
                        <Icon name="bolt" className="w-5 h-5 text-kaspa-primary mb-2" />
                        <p className="text-[10px] text-gray-500 uppercase tracking-widest font-bold text-center">{t('create_match.instant')}</p>
                    </div>
                    <div className="bg-slate-900/60 border border-slate-700/50 rounded-2xl p-4 flex flex-col items-center hover:-translate-y-0.5 transition-transform">
                        <Icon name="lock" className="w-5 h-5 text-kaspa-primary mb-2" />
                        <p className="text-[10px] text-gray-500 uppercase tracking-widest font-bold text-center">{t('create_match.escrow')}</p>
                    </div>
                </div>
            </div>
        </div>
    );
}
