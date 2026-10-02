import { useEffect, useState } from 'react';
import { Link, useSearchParams } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { verifyEmail } from '../api/account';
import { AuthCard, FormAlert } from '../components/auth/AuthCard';
import { useAuthStore } from '../stores/useAuthStore';

// A token is single-use: remember the in-flight request so React StrictMode's double effect (dev)
// or a re-render never consumes it twice and reports a false failure.
const attempts = new Map<string, Promise<unknown>>();

export function VerifyEmailPage() {
    const { t } = useTranslation();
    const [params] = useSearchParams();
    const token = params.get('token') ?? '';
    const fetchUser = useAuthStore((s) => s.fetchUser);
    const [state, setState] = useState<'working' | 'ok' | 'failed'>(token ? 'working' : 'failed');

    useEffect(() => {
        if (!token) return;
        let cancelled = false;
        const attempt = attempts.get(token) ?? verifyEmail(token);
        attempts.set(token, attempt);
        attempt
            .then(() => {
                if (!cancelled) {
                    setState('ok');
                    void fetchUser().catch(() => undefined);
                }
            })
            .catch(() => !cancelled && setState('failed'));
        return () => { cancelled = true; };
    }, [token, fetchUser]);

    return (
        <AuthCard title={t('auth.verify.title')}>
            {state === 'working' && <FormAlert kind="info">{t('common.loading')}</FormAlert>}
            {state === 'ok' && (
                <>
                    <FormAlert kind="success">{t('auth.verify.ok')}</FormAlert>
                    <Link to="/free-play" className="btn-primary flex h-11 items-center justify-center">{t('freeplay.cta_play')}</Link>
                </>
            )}
            {state === 'failed' && (
                <>
                    <FormAlert kind="error">{t('auth.verify.failed')}</FormAlert>
                    <Link to="/login" className="text-sm font-bold text-kaspa-primary hover:underline">{t('auth.login.submit')}</Link>
                </>
            )}
        </AuthCard>
    );
}
