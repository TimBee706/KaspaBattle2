import { useState } from 'react';
import { Link, useNavigate, useSearchParams } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { AccountApiError, loginWithPassword, resendVerification } from '../api/account';
import { AuthCard, FormAlert } from '../components/auth/AuthCard';
import { FormField } from '../components/common/FormField';
import { safeNextPath } from '../utils/safeNextPath';
import { useAuthStore } from '../stores/useAuthStore';

export function LoginPage() {
    const { t } = useTranslation();
    const navigate = useNavigate();
    const [params] = useSearchParams();
    const fetchUser = useAuthStore((s) => s.fetchUser);
    const [email, setEmail] = useState('');
    const [password, setPassword] = useState('');
    const [remember, setRemember] = useState(false);
    const [busy, setBusy] = useState(false);
    const [error, setError] = useState<string | null>(null);
    const [unverified, setUnverified] = useState(false);
    const [resent, setResent] = useState(false);

    const submit = async (e: React.FormEvent) => {
        e.preventDefault();
        if (busy) return;
        setBusy(true);
        setError(null);
        setUnverified(false);
        try {
            await loginWithPassword(email.trim(), password, remember);
            await fetchUser();
            navigate(safeNextPath(params.get('next')), { replace: true });
        } catch (err) {
            const code = (err as AccountApiError).code;
            if (code === 'email_not_verified') {
                setUnverified(true);
            } else if (code === 'rate_limited') {
                setError(t('auth.errors.rate_limited'));
            } else if (code === 'invalid_credentials') {
                setError(t('auth.errors.invalid_credentials'));
            } else {
                setError(t('auth.errors.generic'));
            }
        } finally {
            setBusy(false);
        }
    };

    return (
        <AuthCard
            title={t('auth.login.title')}
            subtitle={t('auth.login.subtitle')}
            footer={<>{t('auth.login.no_account')} <Link to="/register" className="font-bold text-kaspa-primary hover:underline">{t('auth.register.submit_short')}</Link></>}
        >
            <form onSubmit={submit} className="space-y-4" noValidate>
                <FormField label={t('auth.email')} htmlFor="login-email">
                    <input id="login-email" type="email" className="form-input" autoComplete="email" value={email} onChange={(e) => setEmail(e.target.value)} required />
                </FormField>
                <FormField label={t('auth.password')} htmlFor="login-password">
                    <input id="login-password" type="password" className="form-input" autoComplete="current-password" value={password} onChange={(e) => setPassword(e.target.value)} required />
                </FormField>
                <label className="flex items-center gap-3 text-sm text-gray-300">
                    <input type="checkbox" className="h-4 w-4 accent-[#49EACB]" checked={remember} onChange={(e) => setRemember(e.target.checked)} />
                    {t('auth.login.remember')}
                </label>
                {error && <FormAlert kind="error">{error}</FormAlert>}
                {unverified && (
                    <div className="space-y-2">
                        <FormAlert kind="info">{t('auth.errors.email_not_verified')}</FormAlert>
                        {resent ? (
                            <FormAlert kind="success">{t('auth.verify.resent')}</FormAlert>
                        ) : (
                            <button type="button" className="text-sm font-bold text-kaspa-primary hover:underline" onClick={() => void resendVerification(email.trim()).then(() => setResent(true)).catch(() => setResent(true))}>
                                {t('auth.verify.resend')}
                            </button>
                        )}
                    </div>
                )}
                <button type="submit" disabled={busy} className="btn-primary h-12 w-full">
                    {busy ? t('common.loading') : t('auth.login.submit')}
                </button>
                <p className="text-center text-sm">
                    <Link to="/forgot-password" className="text-gray-400 hover:text-kaspa-primary">{t('auth.login.forgot')}</Link>
                </p>
            </form>
        </AuthCard>
    );
}
