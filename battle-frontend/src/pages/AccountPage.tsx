import { useEffect, useState } from 'react';
import { Link, useNavigate } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { AccountApiError, changePassword, resendVerification, setNewsletter } from '../api/account';
import { getFreePlayHistory, getFreePlayStats } from '../api/freePlay';
import type { FpHistoryItem, FpStats } from '../domain/freePlay';
import { useAuthStore } from '../stores/useAuthStore';
import { PageHeader } from '../components/common/PageHeader';
import { FormField } from '../components/common/FormField';
import { FormAlert } from '../components/auth/AuthCard';
import { TestnetComingSoon } from '../components/common/TestnetComingSoon';

function StatTile({ label, value }: { label: string; value: number }) {
    return (
        <div className="rounded-xl border border-kaspa-border bg-kaspa-dark/50 p-4 text-center">
            <div className="text-2xl font-black text-white">{value}</div>
            <div className="mt-1 text-2xs font-bold uppercase tracking-widest text-gray-500">{label}</div>
        </div>
    );
}

export function AccountPage() {
    const { t } = useTranslation();
    const navigate = useNavigate();
    const { user, isAuthenticated, isAuthLoading, fetchUser, logout } = useAuthStore();
    const [stats, setStats] = useState<FpStats | null>(null);
    const [history, setHistory] = useState<FpHistoryItem[]>([]);
    const [current, setCurrent] = useState('');
    const [next, setNext] = useState('');
    const [pwMsg, setPwMsg] = useState<{ kind: 'error' | 'success'; text: string } | null>(null);
    const [resent, setResent] = useState(false);

    useEffect(() => {
        if (!isAuthenticated) return;
        void getFreePlayStats().then(setStats).catch(() => undefined);
        void getFreePlayHistory(20).then(setHistory).catch(() => undefined);
    }, [isAuthenticated]);

    if (isAuthLoading) return null;
    if (!isAuthenticated || !user) {
        return (
            <div className="glass-panel mx-auto max-w-md space-y-4 p-8 text-center">
                <p className="text-gray-300">{t('freeplay.login_required')}</p>
                <Link to="/login?next=/account" className="btn-primary flex h-11 items-center justify-center">{t('auth.login.submit')}</Link>
            </div>
        );
    }

    const name = user.username || user.display_name || '';
    const hasPassword = !!user.has_password_login;

    const submitPassword = async (e: React.FormEvent) => {
        e.preventDefault();
        setPwMsg(null);
        try {
            await changePassword(current, next);
            setCurrent('');
            setNext('');
            setPwMsg({ kind: 'success', text: t('account.password_changed') });
        } catch (err) {
            const e2 = err as AccountApiError;
            setPwMsg({
                kind: 'error',
                text: e2.code === 'invalid_credentials' ? t('account.wrong_password') : e2.code === 'invalid_input' ? t(`auth.field_error.${e2.fields.newPassword ?? 'invalid'}`, { defaultValue: t('auth.field_error.invalid') }) : t('auth.errors.generic'),
            });
        }
    };

    return (
        <div className="mx-auto max-w-3xl space-y-8">
            <PageHeader icon="user" title={name} subtitle={t('account.title')} />

            <section className="glass-panel space-y-3 p-6" aria-labelledby="acc-info">
                <h2 id="acc-info" className="text-sm font-black uppercase tracking-widest text-gray-500">{t('account.details')}</h2>
                <dl className="grid grid-cols-1 gap-3 text-sm sm:grid-cols-2">
                    <div><dt className="text-gray-500">{t('auth.username')}</dt><dd className="font-bold text-white">{name}</dd></div>
                    {hasPassword && (
                        <div><dt className="text-gray-500">{t('auth.email')}</dt><dd className="font-bold text-white break-all" data-testid="acc-email">{user.email}</dd></div>
                    )}
                    {hasPassword && (
                        <div>
                            <dt className="text-gray-500">{t('account.email_status')}</dt>
                            <dd data-testid="acc-verified" className={user.email_verified ? 'font-bold text-emerald-400' : 'font-bold text-amber-400'}>
                                {user.email_verified ? t('account.verified') : t('account.not_verified')}
                            </dd>
                        </div>
                    )}
                    <div><dt className="text-gray-500">{t('account.member_since')}</dt><dd className="font-bold text-white">{user.created_at ? new Date(user.created_at).toLocaleDateString() : '—'}</dd></div>
                </dl>
                {hasPassword && !user.email_verified && user.email && (
                    resent ? <FormAlert kind="success">{t('auth.verify.resent')}</FormAlert> : (
                        <button type="button" className="text-sm font-bold text-kaspa-primary hover:underline" onClick={() => void resendVerification(user.email ?? '').finally(() => setResent(true))}>
                            {t('auth.verify.resend')}
                        </button>
                    )
                )}
            </section>

            <section className="glass-panel space-y-4 p-6" aria-labelledby="acc-stats">
                <h2 id="acc-stats" className="text-sm font-black uppercase tracking-widest text-gray-500">{t('account.stats')}</h2>
                {stats && (
                    <div className="grid grid-cols-2 gap-3 sm:grid-cols-4" data-testid="fp-stats">
                        <StatTile label={t('account.played')} value={stats.played} />
                        <StatTile label={t('account.wins')} value={stats.wins} />
                        <StatTile label={t('account.losses')} value={stats.losses} />
                        <StatTile label={t('account.draws')} value={stats.draws} />
                        <StatTile label={t('account.vs_humans')} value={stats.vsHumans} />
                        <StatTile label={t('account.vs_bots')} value={stats.vsBots} />
                    </div>
                )}
                <h3 className="pt-2 text-xs font-black uppercase tracking-widest text-gray-500">{t('account.history')}</h3>
                {history.length === 0 ? (
                    <p className="text-sm text-gray-500">{t('account.no_games')} <Link to="/free-play" className="font-bold text-kaspa-primary hover:underline">{t('freeplay.cta_play')}</Link></p>
                ) : (
                    <ul className="divide-y divide-kaspa-border" data-testid="fp-history">
                        {history.map((g) => (
                            <li key={g.id} className="flex items-center justify-between gap-3 py-2 text-sm">
                                <span className="text-white">{g.opponentKind === 'bot' ? t('freeplay.bot_label', { level: t(`freeplay.diff.${g.botDifficulty ?? 'easy'}`) }) : g.opponentName}</span>
                                <span className={g.outcome === 'win' ? 'font-bold text-emerald-400' : g.outcome === 'loss' ? 'font-bold text-red-400' : 'font-bold text-gray-400'}>{t(`account.outcome.${g.outcome}`)}</span>
                                <span className="text-xs text-gray-500">{g.finishedAt ? new Date(g.finishedAt).toLocaleString() : ''}</span>
                            </li>
                        ))}
                    </ul>
                )}
            </section>

            <section className="glass-panel space-y-3 p-6" aria-labelledby="acc-news">
                <h2 id="acc-news" className="text-sm font-black uppercase tracking-widest text-gray-500">{t('account.newsletter')}</h2>
                <label className="flex items-start gap-3 text-sm text-gray-300">
                    <input type="checkbox" className="mt-1 h-4 w-4 accent-[#49EACB]" checked={!!user.newsletter_subscribed} onChange={(e) => void setNewsletter(e.target.checked).then(() => fetchUser())} />
                    <span>{t('account.newsletter_text')}</span>
                </label>
            </section>

            {hasPassword && (
                <section className="glass-panel p-6" aria-labelledby="acc-pw">
                    <h2 id="acc-pw" className="mb-4 text-sm font-black uppercase tracking-widest text-gray-500">{t('account.change_password')}</h2>
                    <form onSubmit={submitPassword} className="space-y-4">
                        <FormField label={t('account.current_password')} htmlFor="acc-current">
                            <input id="acc-current" type="password" className="form-input" autoComplete="current-password" value={current} onChange={(e) => setCurrent(e.target.value)} required />
                        </FormField>
                        <FormField label={t('auth.password_new')} htmlFor="acc-new" hint={t('auth.password_hint')}>
                            <input id="acc-new" type="password" className="form-input" autoComplete="new-password" value={next} onChange={(e) => setNext(e.target.value)} required />
                        </FormField>
                        {pwMsg && <FormAlert kind={pwMsg.kind}>{pwMsg.text}</FormAlert>}
                        <button type="submit" className="btn-primary h-11 px-6">{t('account.change_password')}</button>
                    </form>
                </section>
            )}

            <section className="glass-panel space-y-3 p-6" aria-labelledby="acc-link">
                <h2 id="acc-link" className="text-sm font-black uppercase tracking-widest text-gray-500">{t('account.optional_links')}</h2>
                <p className="text-sm text-gray-400">{t('account.optional_links_text')}</p>
                <TestnetComingSoon />
                <div className="flex flex-wrap gap-3 text-sm">
                    <Link to="/wallet/import" className="glass-button px-4 py-2 font-semibold">{t('navigation.wallet')}</Link>
                    <Link to="/profile" className="glass-button px-4 py-2 font-semibold">FACEIT</Link>
                </div>
            </section>

            <button type="button" className="btn-danger h-11 px-6 text-sm font-black uppercase tracking-widest" onClick={() => void logout().then(() => navigate('/'))}>
                {t('account.logout')}
            </button>
        </div>
    );
}
