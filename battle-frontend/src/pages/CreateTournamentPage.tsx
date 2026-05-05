import { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { createTournament } from '../api/tournaments';
import { Icon } from '../components/Icon';

export function CreateTournamentPage() {
    const { t } = useTranslation();
    const navigate = useNavigate();

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
        <div className="container mx-auto px-4 py-8 animate-in fade-in slide-in-from-top-4 duration-500">
            {/* Header */}
            <div className="flex flex-col md:flex-row md:justify-between items-start md:items-center gap-6 mb-12">
                <div>
                    <h1 className="text-4xl font-black text-white uppercase tracking-tighter mb-2">{t('tournaments.create.title')}</h1>
                    <p className="text-slate-500 text-sm font-bold uppercase tracking-widest">{t('tournaments.create.game_hint')}</p>
                </div>
            </div>

            <div className="max-w-lg mx-auto mb-8 p-6 glass-panel rounded-2xl border border-kaspa-primary/20 shadow-glow-primary">
                <h2 className="text-xl font-black mb-6 text-emerald-400 uppercase tracking-tighter pl-2">
                    {t('navigation.create_tournament')}
                </h2>

                <div className="glass-panel p-8">
                    <form onSubmit={handleSubmit} className="space-y-6">
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
                        
                        <div>
                            <label className="block text-xs uppercase tracking-widest text-gray-500 font-bold mb-2">{t('tournaments.create.game_label')}</label>
                            <div className="w-full bg-kaspa-dark/50 border border-kaspa-border rounded-lg px-3 py-2 text-gray-500 text-sm flex items-center justify-between cursor-not-allowed">
                                <span>{t('tournaments.create.game_value')}</span>
                                <span className="text-[10px] uppercase tracking-widest text-gray-600 bg-white/5 px-1.5 py-0.5 rounded">Only option</span>
                            </div>
                        </div>

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
                                <input
                                    id="create-tournament-buyin"
                                    type="number" min={0} step={0.01}
                                    className="w-full bg-kaspa-dark border border-kaspa-border rounded-lg px-3 py-2 text-sm text-white focus:outline-none focus:border-kaspa-primary"
                                    value={form.buy_in_sompi / 1e8}
                                    onChange={e => setForm(f => ({ ...f, buy_in_sompi: Math.round(Number(e.target.value) * 1e8) }))}
                                />
                            </div>
                        </div>

                        <div>
                            <label className="block text-xs uppercase tracking-widest text-gray-500 font-bold mb-2">{t('tournaments.create.prize_split')}</label>
                            <div className="grid grid-cols-3 gap-2">
                                {(['prize_winner_pct', 'prize_runner_up_pct', 'platform_fee_pct'] as const).map((key, i) => (
                                    <div key={key}>
                                        <label className="block text-[10px] uppercase tracking-widest text-gray-500 mb-1 font-bold">
                                            {[t('tournaments.create.winner_pct'), t('tournaments.create.runner_up_pct'), t('tournaments.create.fee_pct')][i]}
                                        </label>
                                        <input
                                            type="number" min={0} max={100}
                                            className="w-full bg-kaspa-dark border border-kaspa-border rounded-lg px-3 py-2 text-sm text-white focus:outline-none focus:border-kaspa-primary"
                                            value={form[key]}
                                            onChange={e => setForm(f => ({ ...f, [key]: Number(e.target.value) }))}
                                        />
                                    </div>
                                ))}
                            </div>
                            {pct !== 100 && (
                                <p className="text-orange-400 text-xs mt-2 font-bold flex items-center gap-1">
                                    <Icon name="alert-triangle" className="w-3.5 h-3.5" />
                                    {t('tournaments.create.pct_warning', { pct })}
                                </p>
                            )}
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

                        {error && (
                            <p className="text-red-400 text-xs font-bold flex items-center gap-1.5 justify-center">
                                <Icon name="x" className="w-3.5 h-3.5 shrink-0" /> {error}
                            </p>
                        )}

                        <button
                            id="create-tournament-submit"
                            type="submit"
                            disabled={loading || pct !== 100 || !form.name}
                            className="w-full btn-primary h-12 relative overflow-hidden group"
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
