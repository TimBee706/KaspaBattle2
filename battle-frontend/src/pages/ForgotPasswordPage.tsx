import { useState } from 'react';
import { Link } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { AccountApiError, forgotPassword } from '../api/account';
import { AuthCard, FormAlert } from '../components/auth/AuthCard';
import { FormField } from '../components/common/FormField';

export function ForgotPasswordPage() {
    const { t } = useTranslation();
    const [email, setEmail] = useState('');
    const [busy, setBusy] = useState(false);
    const [state, setState] = useState<'idle' | 'sent' | 'unavailable' | 'error'>('idle');

    const submit = async (e: React.FormEvent) => {
        e.preventDefault();
        if (busy) return;
        setBusy(true);
        try {
            await forgotPassword(email.trim());
            setState('sent');
        } catch (err) {
            setState((err as AccountApiError).code === 'email_delivery_unavailable' ? 'unavailable' : 'error');
        } finally {
            setBusy(false);
        }
    };

    return (
        <AuthCard
            title={t('auth.forgot.title')}
            subtitle={t('auth.forgot.subtitle')}
            footer={<Link to="/login" className="font-bold text-kaspa-primary hover:underline">{t('auth.login.submit')}</Link>}
        >
            {state === 'sent' ? (
                <FormAlert kind="success">{t('auth.forgot.sent')}</FormAlert>
            ) : (
                <form onSubmit={submit} className="space-y-4" noValidate>
                    <FormField label={t('auth.email')} htmlFor="forgot-email">
                        <input id="forgot-email" type="email" className="form-input" autoComplete="email" value={email} onChange={(e) => setEmail(e.target.value)} required />
                    </FormField>
                    {state === 'unavailable' && <FormAlert kind="error">{t('auth.forgot.unavailable')}</FormAlert>}
                    {state === 'error' && <FormAlert kind="error">{t('auth.errors.generic')}</FormAlert>}
                    <button type="submit" disabled={busy || !email} className="btn-primary h-12 w-full">
                        {busy ? t('common.loading') : t('auth.forgot.submit')}
                    </button>
                </form>
            )}
        </AuthCard>
    );
}
